# Launch Prep Roadmap — Multi-Provider + Cleanup

> **Created:** 2026-02-27
> **Design Doc:** [docs/plans/2026-02-27-multi-provider-launch-prep-design.md](./docs/plans/2026-02-27-multi-provider-launch-prep-design.md)
> **Purpose:** Multi-session roadmap for removing charts/boards and adding OpenAI + Gemini support.
> **How to use:** Each session picks up the next unchecked item. Session start/end commands reference this file.

---

## Phase A: Charts/Boards Removal (1 session)

**Goal:** Remove all analytics, charts, insight boards, and recharts dependency.

- [x] A.1 Delete frontend analytics + insights components and type files
- [x] A.2 Remove AnalyticsChart from MessageBubble.tsx, InsightBoardView from App.tsx, InsightBoardPanel from AppShell.tsx
- [x] A.3 Remove recharts from package.json, run `npm install`
- [x] A.4 Delete Rust modules: analytics.rs, analytics_templates.rs, insight_canvas.rs
- [x] A.5 Remove 11 analytics/canvas Tauri commands from lib.rs
- [x] A.6 Remove 11 analytics/canvas wrappers from tauri-commands.ts
- [x] A.7 Remove or comment out analytics parsing in ConversationContext.tsx + context.rs
- [x] A.8 Add migration to drop insight_boards, pinned_charts, chart_annotations tables
- [x] A.9 Verify: `cargo test`, `npx tsc --noEmit`, `npm run build` all pass
- [ ] A.10 Commit: "[Launch Prep] Remove charts, boards, and analytics — focus on conversational HR"

### Pause Point A ─ Verify
- [x] App builds and launches cleanly
- [x] Chat works end-to-end (no broken imports)
- [x] Test count: 302 (down from 317, lost 15 analytics tests)

---

## Phase B: Provider Trait + Anthropic Extraction (1 session)

**Goal:** Create the provider abstraction layer and extract existing Claude logic into it. No new providers yet — just the refactor. All existing tests must still pass.

- [ ] B.1 Create `src-tauri/src/provider.rs` with Provider trait and enum
- [ ] B.2 Create `src-tauri/src/providers/mod.rs`
- [ ] B.3 Create `src-tauri/src/providers/anthropic.rs` — extract from chat.rs
- [ ] B.4 Refactor chat.rs to use provider trait for request building + response parsing
- [ ] B.5 Refactor chat.rs streaming to use provider trait for SSE event parsing
- [ ] B.6 Extend keyring.rs for per-provider key storage (keep backward compat)
- [ ] B.7 Add `active_provider` to settings (default: "anthropic")
- [ ] B.8 Update lib.rs commands to route through provider abstraction
- [ ] B.9 Migrate existing chat tests to work with new abstraction
- [ ] B.10 Verify: all existing tests pass, chat works identically

### Pause Point B ─ Verify
- [ ] Existing behavior unchanged (pure refactor)
- [ ] `cargo test` passes with same count as Phase A
- [ ] Chat still works through both trial (proxy) and BYOK paths

---

## Phase C: OpenAI Provider (1 session)

**Goal:** Implement OpenAI adapter. User can paste an OpenAI key and chat works.

- [ ] C.1 Create `src-tauri/src/providers/openai.rs` implementing Provider trait
- [ ] C.2 Implement build_request() — OpenAI message format, Bearer auth
- [ ] C.3 Implement parse_response() — OpenAI response → internal format
- [ ] C.4 Implement parse_stream_event() — `chat.completion.chunk` SSE events
- [ ] C.5 Implement validate_key_format() — `sk-` prefix
- [ ] C.6 Add provider_hints() for OpenAI (system prompt suffix if needed)
- [ ] C.7 Set model + endpoint constants (gpt-4o, max tokens, API URL)
- [ ] C.8 Unit tests: request building, response parsing, stream parsing, key validation
- [ ] C.9 Verify: `cargo test` passes, manually test with real OpenAI key

### Pause Point C ─ Verify
- [ ] Can send a message using OpenAI key and get a response
- [ ] Streaming works (SSE events parsed correctly)
- [ ] Alex persona and HR context come through in responses

---

## Phase D: Gemini Provider (1 session)

**Goal:** Implement Gemini adapter. User can paste a Gemini key and chat works.

- [ ] D.1 Create `src-tauri/src/providers/gemini.rs` implementing Provider trait
- [ ] D.2 Implement build_request() — Gemini message format, system_instruction field
- [ ] D.3 Implement parse_response() — Gemini response → internal format
- [ ] D.4 Implement parse_stream_event() — Gemini streaming format
- [ ] D.5 Implement validate_key_format() — Gemini key format
- [ ] D.6 Add provider_hints() for Gemini (system prompt suffix if needed)
- [ ] D.7 Set model + endpoint constants (gemini-2.0-flash or similar, API URL)
- [ ] D.8 Unit tests: request building, response parsing, stream parsing, key validation
- [ ] D.9 Verify: `cargo test` passes, manually test with real Gemini key

### Pause Point D ─ Verify
- [ ] Can send a message using Gemini key and get a response
- [ ] Streaming works
- [ ] Alex persona and HR context come through in responses

---

## Phase E: Frontend — Provider Picker + Updated Onboarding (1-2 sessions)

**Goal:** Users can choose their provider and set up API keys through a polished flow.

### E.1 Provider Picker Component
- [ ] E.1.1 Create `ProviderPicker.tsx` — card-style selector (Claude / OpenAI / Gemini)
- [ ] E.1.2 Add provider logos/icons and "what you need" one-liners
- [ ] E.1.3 Wire selection to settings (store active_provider)

### E.2 Updated API Key Setup
- [ ] E.2.1 Refactor `ApiKeyStep.tsx` → `ProviderSetupStep.tsx` (provider picker → key input)
- [ ] E.2.2 Update `ApiKeyInput.tsx` to validate per selected provider
- [ ] E.2.3 Create per-provider setup guides (collapsible sections with console URLs, steps)

### E.3 Settings Panel
- [ ] E.3.1 Add provider section to SettingsPanel (current provider, switch, manage keys)
- [ ] E.3.2 Show which providers have keys stored

### E.4 Upgrade Flow
- [ ] E.4.1 Update UpgradePrompt → after license entry, route to ProviderPicker
- [ ] E.4.2 Trial → upgrade → provider pick → key setup → full access

### E.5 Verification
- [ ] E.5.1 `npx tsc --noEmit` and `npm run build` pass
- [ ] E.5.2 Walk through: fresh install → trial → upgrade → pick provider → enter key → chat

### Pause Point E ─ Verify
- [ ] Full onboarding flow works for all three providers
- [ ] Settings provider switching works
- [ ] Trial → upgrade path is smooth

---

## Phase F: Final Integration + Launch Ready (1 session)

**Goal:** Polish, final E2E testing, release prep.

- [ ] F.1 Test trial chat against live proxy
- [ ] F.2 Test upgrade flow end-to-end (purchase → license → provider → key → chat)
- [ ] F.3 Test each provider with real API keys
- [ ] F.4 Update features.json with new feature statuses
- [ ] F.5 Update ROADMAP.md phase status
- [ ] F.6 Final `cargo test` + `npx tsc --noEmit` + `npm run build`
- [ ] F.7 Prep first release build

### Pause Point F ─ Launch Ready
- [ ] All providers work end-to-end
- [ ] Trial → paid flow works for all providers
- [ ] Build succeeds, tests pass
- [ ] Ready for `cargo tauri build` or GitHub Actions release

---

## Session Quick Reference

**Start of session:**
```
1. Run ./scripts/dev-init.sh
2. Read ROADMAP_LAUNCH_PREP.md — find next unchecked phase
3. Read docs/PROGRESS.md — last session context
4. Begin work on next unchecked item
```

**End of session:**
```
1. cargo test + npx tsc --noEmit + npm run build
2. Check off completed items in ROADMAP_LAUNCH_PREP.md
3. Add session entry to docs/PROGRESS.md (reference phase letter)
4. Commit with "[Launch Prep Phase X] description"
```
