// HR Command Center — Cloudflare Workers API Proxy
// Forwards trial-mode chat requests to Claude API with per-device quota tracking.

interface Env {
  QUOTA: KVNamespace;
  CLAUDE_API_KEY: string;
  MAX_MESSAGES: string;
  ALLOWED_MODEL: string;
}

const ANTHROPIC_API_URL = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION = "2023-06-01";
const DEFAULT_ALLOWED_MODEL = "claude-sonnet-4-20250514";
const MAX_TOKENS_CAP = 4096;

const UUID_V4_REGEX =
  /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;

const CORS_HEADERS: Record<string, string> = {
  "Access-Control-Allow-Origin": "*",
  "Access-Control-Allow-Methods": "POST, OPTIONS",
  "Access-Control-Allow-Headers": "Content-Type, X-Device-ID",
  "Access-Control-Max-Age": "86400",
};

function corsResponse(body: string | null, status: number, extra?: Record<string, string>): Response {
  return new Response(body, {
    status,
    headers: { ...CORS_HEADERS, "Content-Type": "application/json", ...extra },
  });
}

function errorJson(error: string, message: string, status: number, extra?: Record<string, unknown>): Response {
  return corsResponse(JSON.stringify({ error, message, ...extra }), status);
}

export default {
  async fetch(request: Request, env: Env, ctx: ExecutionContext): Promise<Response> {
    // CORS preflight
    if (request.method === "OPTIONS") {
      return new Response(null, { status: 204, headers: CORS_HEADERS });
    }

    // Only POST /v1/messages is supported
    const url = new URL(request.url);
    if (url.pathname !== "/v1/messages") {
      return errorJson("not_found", "Only POST /v1/messages is supported", 404);
    }
    if (request.method !== "POST") {
      return errorJson("method_not_allowed", "Only POST is allowed", 405);
    }

    // --- Validate device ID ---
    const deviceId = request.headers.get("X-Device-ID");
    if (!deviceId || !UUID_V4_REGEX.test(deviceId)) {
      return errorJson(
        "invalid_device_id",
        "X-Device-ID header must be a valid UUID v4",
        400,
      );
    }

    // --- Check quota ---
    const maxMessages = parseInt(env.MAX_MESSAGES, 10) || 50;
    const countStr = await env.QUOTA.get(deviceId);
    const used = countStr ? parseInt(countStr, 10) : 0;

    if (used >= maxMessages) {
      return errorJson(
        "trial_limit_reached",
        "You have used all your free trial messages. Add your own API key to continue.",
        402,
        { used, limit: maxMessages },
      );
    }

    // --- Parse and validate request body ---
    let body: Record<string, unknown>;
    try {
      body = await request.json() as Record<string, unknown>;
    } catch {
      return errorJson("invalid_body", "Request body must be valid JSON", 400);
    }

    if (!body.messages || !Array.isArray(body.messages) || body.messages.length === 0) {
      return errorJson("invalid_body", "Request must include a non-empty messages array", 400);
    }

    // Override model and cap max_tokens to prevent abuse
    body.model = env.ALLOWED_MODEL || DEFAULT_ALLOWED_MODEL;
    const requestedMaxTokens = typeof body.max_tokens === "number" ? body.max_tokens : MAX_TOKENS_CAP;
    body.max_tokens = Math.min(requestedMaxTokens as number, MAX_TOKENS_CAP);

    const isStreaming = body.stream === true;

    // --- Forward to Claude API ---
    const apiResponse = await fetch(ANTHROPIC_API_URL, {
      method: "POST",
      headers: {
        "x-api-key": env.CLAUDE_API_KEY,
        "anthropic-version": ANTHROPIC_VERSION,
        "content-type": "application/json",
      },
      body: JSON.stringify(body),
    });

    // If Claude API returned an error, pass it through without incrementing count
    if (!apiResponse.ok) {
      const errBody = await apiResponse.text();
      return corsResponse(errBody, apiResponse.status);
    }

    // --- Increment quota (don't await — fire and forget so streaming isn't delayed) ---
    const incrementPromise = env.QUOTA.put(deviceId, String(used + 1));

    // --- Return response ---
    if (isStreaming) {
      // Pass through the SSE stream directly
      const responseHeaders = new Headers(CORS_HEADERS);
      responseHeaders.set("Content-Type", "text/event-stream");
      responseHeaders.set("Cache-Control", "no-cache");
      responseHeaders.set("Connection", "keep-alive");

      // Ensure KV write completes even after response is sent
      ctx.waitUntil(incrementPromise);

      return new Response(apiResponse.body, {
        status: 200,
        headers: responseHeaders,
      });
    }

    // Non-streaming: wait for KV write, return JSON
    await incrementPromise;
    const responseBody = await apiResponse.text();
    return corsResponse(responseBody, 200);
  },
} satisfies ExportedHandler<Env>;
