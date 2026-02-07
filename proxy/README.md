# HR Command Center — API Proxy

Cloudflare Workers proxy that provides 50 free trial messages by forwarding requests to the Claude API with per-device quota tracking.

## Setup

```bash
cd proxy
npm install
```

## Configuration

### 1. Create KV Namespace

```bash
wrangler kv:namespace create QUOTA
wrangler kv:namespace create QUOTA --preview
```

Copy the namespace IDs into `wrangler.toml`:

```toml
kv_namespaces = [
  { binding = "QUOTA", id = "<production-id>", preview_id = "<preview-id>" }
]
```

### 2. Set API Key Secret

```bash
wrangler secret put CLAUDE_API_KEY
```

Paste your Anthropic API key when prompted. This is stored securely in Cloudflare and never exposed in code or config.

### 3. Adjust Quota (Optional)

Edit `MAX_MESSAGES` in `wrangler.toml` to change the trial message limit (default: 50).

## Development

```bash
npm run dev
```

This starts a local dev server. Test with:

```bash
curl -X POST http://localhost:8787/v1/messages \
  -H "Content-Type: application/json" \
  -H "X-Device-ID: 550e8400-e29b-41d4-a716-446655440000" \
  -d '{
    "model": "claude-sonnet-4-20250514",
    "max_tokens": 256,
    "messages": [{"role": "user", "content": "Hello"}]
  }'
```

## Deployment

```bash
npm run deploy
```

The worker URL will be printed after deployment. Update the Tauri app's proxy URL configuration to point to it.

## Architecture

```
[Tauri App] --X-Device-ID--> [Cloudflare Worker] --x-api-key--> [Claude API]
                                     |
                              [KV: QUOTA store]
                              device_id -> message_count
```

- **Quota tracking:** KV stores a simple counter per device UUID
- **Streaming:** SSE responses are passed through directly (no buffering)
- **Security:** API key is a Cloudflare secret, model is force-overridden, max_tokens is capped
- **Privacy:** No PII logging; messages are already PII-redacted by the app before sending
