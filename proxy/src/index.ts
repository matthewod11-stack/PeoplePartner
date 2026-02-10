// HR Command Center - Cloudflare Workers API Proxy
// Forwards trial-mode chat requests to Claude API with quota and abuse controls.

interface Env {
  QUOTA: KVNamespace;
  CLAUDE_API_KEY: string;
  MAX_MESSAGES: string;
  ALLOWED_MODEL: string;
  TRIAL_SIGNING_SECRET?: string;
  ALLOWED_ORIGINS?: string;
  MAX_SIGNATURE_AGE_SECONDS?: string;
  MAX_IP_REQUESTS_PER_HOUR?: string;
}

const ANTHROPIC_API_URL = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION = "2023-06-01";
const DEFAULT_ALLOWED_MODEL = "claude-sonnet-4-20250514";
const MAX_TOKENS_CAP = 4096;
const DEFAULT_ALLOWED_ORIGINS = ["tauri://localhost", "http://localhost:1420"];
const DEFAULT_SIGNATURE_AGE_SECONDS = 300;
const DEFAULT_MAX_IP_REQUESTS_PER_HOUR = 300;

const UUID_V4_REGEX =
  /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;

function parseAllowedOrigins(value: string | undefined): Set<string> {
  const parsed = (value ?? "")
    .split(",")
    .map((v) => v.trim())
    .filter(Boolean);
  return new Set(parsed.length > 0 ? parsed : DEFAULT_ALLOWED_ORIGINS);
}

function buildCorsHeaders(origin: string): Record<string, string> {
  return {
    "Access-Control-Allow-Origin": origin,
    "Access-Control-Allow-Methods": "POST, OPTIONS",
    "Access-Control-Allow-Headers": "Content-Type, X-Device-ID, X-Trial-Timestamp, X-Trial-Signature",
    "Access-Control-Max-Age": "86400",
    "Vary": "Origin",
  };
}

function trialUsageHeaders(used: number, limit: number): Record<string, string> {
  return {
    "X-Trial-Used": String(used),
    "X-Trial-Limit": String(limit),
  };
}

function corsResponse(
  body: string | null,
  status: number,
  corsHeaders: Record<string, string>,
  contentType = "application/json",
  extraHeaders?: Record<string, string>,
): Response {
  return new Response(body, {
    status,
    headers: { ...corsHeaders, "Content-Type": contentType, ...extraHeaders },
  });
}

function errorJson(
  error: string,
  message: string,
  status: number,
  corsHeaders: Record<string, string>,
  extraBody?: Record<string, unknown>,
  extraHeaders?: Record<string, string>,
): Response {
  return corsResponse(
    JSON.stringify({ error, message, ...(extraBody ?? {}) }),
    status,
    corsHeaders,
    "application/json",
    extraHeaders,
  );
}

async function hmacSha256Hex(secret: string, payload: string): Promise<string> {
  const key = await crypto.subtle.importKey(
    "raw",
    new TextEncoder().encode(secret),
    { name: "HMAC", hash: "SHA-256" },
    false,
    ["sign"],
  );
  const signature = await crypto.subtle.sign(
    "HMAC",
    key,
    new TextEncoder().encode(payload),
  );
  return Array.from(new Uint8Array(signature))
    .map((byte) => byte.toString(16).padStart(2, "0"))
    .join("");
}

function timingSafeEqual(a: string, b: string): boolean {
  if (a.length !== b.length) return false;
  let mismatch = 0;
  for (let i = 0; i < a.length; i += 1) {
    mismatch |= a.charCodeAt(i) ^ b.charCodeAt(i);
  }
  return mismatch === 0;
}

export default {
  async fetch(request: Request, env: Env, ctx: ExecutionContext): Promise<Response> {
    const allowedOrigins = parseAllowedOrigins(env.ALLOWED_ORIGINS);
    const origin = request.headers.get("Origin") ?? "";
    const corsHeaders = buildCorsHeaders(origin || DEFAULT_ALLOWED_ORIGINS[0]);

    if (!origin || !allowedOrigins.has(origin)) {
      return errorJson(
        "invalid_origin",
        "Origin is not allowed for this endpoint",
        403,
        corsHeaders,
      );
    }

    // CORS preflight
    if (request.method === "OPTIONS") {
      return new Response(null, { status: 204, headers: corsHeaders });
    }

    // Only POST /v1/messages is supported
    const url = new URL(request.url);
    if (url.pathname !== "/v1/messages") {
      return errorJson(
        "not_found",
        "Only POST /v1/messages is supported",
        404,
        corsHeaders,
      );
    }
    if (request.method !== "POST") {
      return errorJson("method_not_allowed", "Only POST is allowed", 405, corsHeaders);
    }

    // Validate device ID
    const deviceId = request.headers.get("X-Device-ID");
    if (!deviceId || !UUID_V4_REGEX.test(deviceId)) {
      return errorJson(
        "invalid_device_id",
        "X-Device-ID header must be a valid UUID v4",
        400,
        corsHeaders,
      );
    }

    // IP rate limit (coarse protection against scripted abuse).
    const ip = request.headers.get("CF-Connecting-IP") ?? "unknown";
    const maxIpRequests = parseInt(env.MAX_IP_REQUESTS_PER_HOUR ?? "", 10) || DEFAULT_MAX_IP_REQUESTS_PER_HOUR;
    const hourBucket = Math.floor(Date.now() / 1000 / 3600);
    const ipKey = `ip:${ip}:${hourBucket}`;
    const ipCount = parseInt((await env.QUOTA.get(ipKey)) ?? "0", 10) || 0;
    if (ipCount >= maxIpRequests) {
      return errorJson(
        "rate_limited",
        "Too many requests from this IP. Please try again later.",
        429,
        corsHeaders,
        { used: ipCount, limit: maxIpRequests },
      );
    }
    ctx.waitUntil(env.QUOTA.put(ipKey, String(ipCount + 1), { expirationTtl: 7200 }));

    // Check device quota
    const maxMessages = parseInt(env.MAX_MESSAGES, 10) || 50;
    const countStr = await env.QUOTA.get(deviceId);
    const used = countStr ? parseInt(countStr, 10) : 0;
    if (used >= maxMessages) {
      return errorJson(
        "trial_limit_reached",
        "You have used all your free trial messages. Add your own API key to continue.",
        402,
        corsHeaders,
        { used, limit: maxMessages },
        trialUsageHeaders(used, maxMessages),
      );
    }

    // Parse request body as raw text first (used for signature verification).
    let rawBody: string;
    try {
      rawBody = await request.text();
    } catch {
      return errorJson("invalid_body", "Request body must be valid JSON", 400, corsHeaders);
    }

    const signingSecret = env.TRIAL_SIGNING_SECRET?.trim();
    if (signingSecret) {
      const timestampHeader = request.headers.get("X-Trial-Timestamp");
      const signatureHeader = request.headers.get("X-Trial-Signature");
      if (!timestampHeader || !signatureHeader) {
        return errorJson(
          "missing_signature",
          "Signed request headers are required",
          401,
          corsHeaders,
        );
      }

      const timestamp = Number(timestampHeader);
      const nowSeconds = Math.floor(Date.now() / 1000);
      const maxAge = parseInt(env.MAX_SIGNATURE_AGE_SECONDS ?? "", 10) || DEFAULT_SIGNATURE_AGE_SECONDS;
      if (!Number.isFinite(timestamp) || Math.abs(nowSeconds - timestamp) > maxAge) {
        return errorJson(
          "stale_signature",
          "Request signature has expired",
          401,
          corsHeaders,
        );
      }

      const payload = `${deviceId}:${timestampHeader}:${rawBody}`;
      const expected = await hmacSha256Hex(signingSecret, payload);
      const provided = signatureHeader.toLowerCase();
      if (!timingSafeEqual(expected, provided)) {
        return errorJson(
          "invalid_signature",
          "Request signature is invalid",
          401,
          corsHeaders,
        );
      }

      const replayKey = `sig:${deviceId}:${timestampHeader}:${provided}`;
      if (await env.QUOTA.get(replayKey)) {
        return errorJson(
          "replay_detected",
          "This request was already processed",
          409,
          corsHeaders,
        );
      }
      ctx.waitUntil(env.QUOTA.put(replayKey, "1", { expirationTtl: maxAge + 60 }));
    }

    // Parse and validate request body JSON
    let body: Record<string, unknown>;
    try {
      body = JSON.parse(rawBody) as Record<string, unknown>;
    } catch {
      return errorJson("invalid_body", "Request body must be valid JSON", 400, corsHeaders);
    }

    if (!body.messages || !Array.isArray(body.messages) || body.messages.length === 0) {
      return errorJson(
        "invalid_body",
        "Request must include a non-empty messages array",
        400,
        corsHeaders,
      );
    }

    // Override model and cap max_tokens to prevent abuse.
    body.model = env.ALLOWED_MODEL || DEFAULT_ALLOWED_MODEL;
    const requestedMaxTokens = typeof body.max_tokens === "number" ? body.max_tokens : MAX_TOKENS_CAP;
    body.max_tokens = Math.min(requestedMaxTokens as number, MAX_TOKENS_CAP);
    const isStreaming = body.stream === true;

    // Forward to Claude API
    const apiResponse = await fetch(ANTHROPIC_API_URL, {
      method: "POST",
      headers: {
        "x-api-key": env.CLAUDE_API_KEY,
        "anthropic-version": ANTHROPIC_VERSION,
        "content-type": "application/json",
      },
      body: JSON.stringify(body),
    });

    // Pass API errors through without incrementing quota.
    if (!apiResponse.ok) {
      const errBody = await apiResponse.text();
      return corsResponse(
        errBody,
        apiResponse.status,
        corsHeaders,
        "application/json",
        trialUsageHeaders(used, maxMessages),
      );
    }

    // Increment quota after successful upstream response.
    const nextUsed = used + 1;
    const incrementPromise = env.QUOTA.put(deviceId, String(nextUsed));

    if (isStreaming) {
      const responseHeaders = new Headers({
        ...corsHeaders,
        ...trialUsageHeaders(nextUsed, maxMessages),
        "Content-Type": "text/event-stream",
        "Cache-Control": "no-cache",
        "Connection": "keep-alive",
      });

      ctx.waitUntil(incrementPromise);

      return new Response(apiResponse.body, {
        status: 200,
        headers: responseHeaders,
      });
    }

    await incrementPromise;
    const responseBody = await apiResponse.text();
    return corsResponse(
      responseBody,
      200,
      corsHeaders,
      "application/json",
      trialUsageHeaders(nextUsed, maxMessages),
    );
  },
} satisfies ExportedHandler<Env>;
