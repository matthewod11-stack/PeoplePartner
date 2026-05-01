#!/usr/bin/env bash
# Trial proxy hardening verification.
# Run AFTER `wrangler secret put TRIAL_SIGNING_SECRET` has been executed.
#
# Tests that the proxy correctly rejects:
#   - unsigned requests        (expect 401 missing_signature)
#   - stale-timestamp requests (expect 401 stale_signature)
#   - wrong-secret requests    (expect 401 invalid_signature)
#   - replayed requests        (expect 409 replay_detected)
# And accepts:
#   - valid signed requests    (expect 200)
#
# Usage:
#   PROXY_URL=https://hrcommand-proxy.hrcommand.workers.dev \
#   TRIAL_SIGNING_SECRET=<the-secret-value> \
#   ./scripts/verify-hardening.sh
#
# Exits 0 on all-pass, 1 on any failure.

set -euo pipefail

PROXY_URL="${PROXY_URL:-https://hrcommand-proxy.hrcommand.workers.dev}"
SECRET="${TRIAL_SIGNING_SECRET:-}"

if [ -z "$SECRET" ]; then
  echo "FAIL: TRIAL_SIGNING_SECRET env var is required."
  echo "Pass the value you set with 'wrangler secret put TRIAL_SIGNING_SECRET'."
  exit 1
fi

DEVICE_ID="$(uuidgen | tr '[:upper:]' '[:lower:]')"
ENDPOINT="${PROXY_URL%/}/v1/messages"
ORIGIN="tauri://localhost"
BODY='{"messages":[{"role":"user","content":"verify"}]}'

PASS=0
FAIL=0

check() {
  local name="$1"
  local expected_status="$2"
  local expected_body_substring="$3"
  local actual_status="$4"
  local actual_body="$5"

  if [ "$actual_status" = "$expected_status" ] && \
     echo "$actual_body" | grep -q "$expected_body_substring"; then
    echo "  PASS — $name (HTTP $actual_status)"
    PASS=$((PASS + 1))
  else
    echo "  FAIL — $name"
    echo "         expected: HTTP $expected_status containing '$expected_body_substring'"
    echo "         got:      HTTP $actual_status — body: $actual_body"
    FAIL=$((FAIL + 1))
  fi
}

# Helper: HMAC-SHA256 hex of "$device:$timestamp:$body" using $secret
sign() {
  local secret="$1"
  local payload="$2"
  printf '%s' "$payload" | openssl dgst -sha256 -mac HMAC -macopt "key:$secret" -hex | awk '{print $2}'
}

# Helper: do a request, capture status + body
request() {
  local timestamp="$1"
  local signature="$2"
  local extra_headers=()
  if [ -n "$timestamp" ]; then
    extra_headers+=(-H "X-Trial-Timestamp: $timestamp")
  fi
  if [ -n "$signature" ]; then
    extra_headers+=(-H "X-Trial-Signature: $signature")
  fi
  curl -sS -o /tmp/_pp_proxy_body -w '%{http_code}' \
    -X POST "$ENDPOINT" \
    -H "Origin: $ORIGIN" \
    -H "Content-Type: application/json" \
    -H "X-Device-ID: $DEVICE_ID" \
    "${extra_headers[@]}" \
    --data "$BODY"
}

echo "Verifying $ENDPOINT"
echo "Device ID: $DEVICE_ID"
echo

# 1. Unsigned — should reject
echo "Test 1: unsigned request"
status=$(request "" "")
body=$(cat /tmp/_pp_proxy_body)
check "unsigned rejected" 401 "missing_signature" "$status" "$body"

# 2. Stale timestamp (10 minutes old) — should reject
echo "Test 2: stale signature"
ts_stale=$(( $(date +%s) - 600 ))
sig_stale=$(sign "$SECRET" "$DEVICE_ID:$ts_stale:$BODY")
status=$(request "$ts_stale" "$sig_stale")
body=$(cat /tmp/_pp_proxy_body)
check "stale signature rejected" 401 "stale_signature" "$status" "$body"

# 3. Wrong secret — should reject
echo "Test 3: wrong secret"
ts_now=$(date +%s)
sig_wrong=$(sign "wrong-secret-value-here" "$DEVICE_ID:$ts_now:$BODY")
status=$(request "$ts_now" "$sig_wrong")
body=$(cat /tmp/_pp_proxy_body)
check "wrong-secret rejected" 401 "invalid_signature" "$status" "$body"

# 4. Valid signed — should accept (or hit upstream Anthropic)
echo "Test 4: valid signed request"
ts_now=$(date +%s)
sig_ok=$(sign "$SECRET" "$DEVICE_ID:$ts_now:$BODY")
status=$(request "$ts_now" "$sig_ok")
body=$(cat /tmp/_pp_proxy_body)
# Accept either 200 (Anthropic returned content) or 200/4xx/5xx pass-through.
# The key check is that we did NOT get a Worker-side 401/409.
if [ "$status" = "200" ] || [ "$status" = "402" ] || [ "$status" = "429" ]; then
  echo "  PASS — valid signed request reached upstream (HTTP $status)"
  PASS=$((PASS + 1))
else
  echo "  FAIL — valid signed request was rejected by Worker"
  echo "         got: HTTP $status — body: $body"
  FAIL=$((FAIL + 1))
fi

# 5. Replay (reuse timestamp + signature from Test 4) — should reject
echo "Test 5: replay protection"
status=$(request "$ts_now" "$sig_ok")
body=$(cat /tmp/_pp_proxy_body)
check "replay rejected" 409 "replay_detected" "$status" "$body"

echo
echo "----"
echo "Results: $PASS passed, $FAIL failed"
if [ "$FAIL" -gt 0 ]; then
  exit 1
fi
exit 0
