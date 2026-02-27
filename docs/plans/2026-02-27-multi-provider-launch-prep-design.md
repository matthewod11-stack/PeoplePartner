# Design: Multi-Provider Support + Launch Prep

> **Date:** 2026-02-27
> **Status:** Approved
> **Scope:** Remove charts/boards, add OpenAI + Gemini provider support, update onboarding

---

## Motivation

The app's core value is the conversational HR assistant. Charts/analytics and insight boards were V2 scope creep that aren't working reliably and distract from the value play. Meanwhile, locking users to Anthropic-only API keys limits the addressable market — many users already have OpenAI or Google accounts.

### Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Charts/Boards | Full removal | Clean cut, smaller bundle, git history preserves code |
| Provider selection UX | Provider picker after upgrade | Trial sells the product first; provider choice at purchase |
| Trial provider | Claude only (proxy) | Single proxy key, predictable costs |
| System prompt | Shared base + provider hints | Consistent Alex persona, light provider-specific tuning |

---

## Part 1: Charts/Boards Full Removal

### Deleted Files (~5,400 lines)

**Frontend:**
- `src/components/analytics/` (8 files, ~1,590 lines)
- `src/components/insights/` (7 files, ~1,396 lines)
- `src/lib/analytics-types.ts` (~159 lines)
- `src/lib/insight-canvas-types.ts` (~148 lines)

**Backend:**
- `src-tauri/src/analytics.rs` (~504 lines)
- `src-tauri/src/analytics_templates.rs` (~1,064 lines)
- `src-tauri/src/insight_canvas.rs` (~542 lines)

**Database:**
- Migration `004_insight_canvas.sql` (tables: `insight_boards`, `pinned_charts`, `chart_annotations`)

**Dependencies:**
- `recharts` npm package removed

### Patched Files

- `MessageBubble.tsx` — Remove AnalyticsChart import + chart rendering block
- `App.tsx` — Remove InsightBoardView lazy import, selectedBoardId state, board JSX
- `AppShell.tsx` — Remove onBoardSelect prop, InsightBoardPanel from sidebar
- `chat.rs` — Remove analytics request parsing from response handler
- `lib.rs` — Remove 11 analytics/canvas command registrations
- `tauri-commands.ts` — Remove 11 analytics/canvas command wrappers

### Untouched

Employee management, chat, memory, PII scanning, onboarding, trial system, settings — all independent.

---

## Part 2: Provider Abstraction Layer

### Backend Architecture

```
Provider trait (provider.rs)
├── AnthropicProvider (providers/anthropic.rs) — extracted from chat.rs
├── OpenAIProvider (providers/openai.rs) — new
└── GeminiProvider (providers/gemini.rs) — new
```

**Trait interface:**
- `build_request()` — Internal ChatMessage → provider-specific JSON
- `parse_response()` — Provider response → internal format
- `parse_stream_event()` — SSE differences per provider
- `api_endpoint()` — Provider URL
- `auth_headers()` — Provider-specific auth headers
- `validate_key_format()` — Key prefix validation
- `max_context_tokens()` — Token budget per model
- `provider_hints()` — Optional system prompt additions

### Message Format Translation

| Internal | Claude | OpenAI | Gemini |
|----------|--------|--------|--------|
| `system` prompt | Top-level `system` field | `messages[0].role = "system"` | `system_instruction` field |
| `user` / `assistant` roles | Same | Same | `user` / `model` |
| Streaming events | `content_block_delta` | `chat.completion.chunk` | `generateContent` stream |

### New Files

- `src-tauri/src/provider.rs` — Trait definition, `Provider` enum, routing logic
- `src-tauri/src/providers/mod.rs` — Module declarations
- `src-tauri/src/providers/anthropic.rs` — Extracted from current chat.rs
- `src-tauri/src/providers/openai.rs` — New implementation
- `src-tauri/src/providers/gemini.rs` — New implementation

### Modified Backend Files

- `chat.rs` — Slim to orchestration (get provider → delegate to adapter)
- `keyring.rs` — Per-provider key storage (`anthropic_api_key`, `openai_api_key`, `gemini_api_key`)
- `settings.rs` — Store selected provider preference
- `lib.rs` — Update command signatures for provider context

### Keychain Storage

```
com.hrcommandcenter.app / anthropic_api_key  (existing)
com.hrcommandcenter.app / openai_api_key     (new)
com.hrcommandcenter.app / gemini_api_key     (new)
com.hrcommandcenter.app / active_provider    (new — "anthropic" | "openai" | "gemini")
```

---

## Part 3: Frontend — Provider Setup Flow

### New Components

- `ProviderPicker.tsx` — Card selector (Claude / OpenAI / Gemini) with logo, name, one-liner
- Per-provider setup guides (collapsible, replaces Anthropic-only guide)

### Modified Components

- `ApiKeyStep.tsx` → `ProviderSetupStep.tsx` — Provider picker first, then key input
- `ApiKeyInput.tsx` — Accept + validate key for any provider
- `SettingsPanel.tsx` — Add provider management section
- `UpgradePrompt.tsx` — After license entry, route to provider picker

### User Flow

```
Trial (free, Claude via proxy)
    → Hit message limit
    → UpgradePrompt
    → Enter license key
    → ProviderPicker (Claude / OpenAI / Gemini)
    → Per-provider API key guide + input
    → Full access with chosen provider
```

Settings allows switching provider at any time after initial setup.

### System Prompt Strategy

- `context.rs` builds provider-agnostic base prompt (Alex persona, HR context, company info)
- Each provider adapter appends optional `provider_hints()` suffix
- Hints are minimal formatting guidance, not behavioral changes

---

## Testing Strategy

- Extract existing chat.rs tests into Anthropic adapter tests
- Add unit tests for each new provider adapter (request building, response parsing, stream parsing)
- Add key format validation tests per provider
- Integration test: provider routing (mock provider → verify correct adapter called)
- Manual E2E: test with real keys for each provider
