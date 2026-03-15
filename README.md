# People Partner

> Your company's HR brain—private, always in context, always ready to help.

A desktop AI assistant for HR professionals that keeps your employee data local while providing intelligent, context-aware guidance.

---

## What It Does

- **Knows Your Company** — Import employee data once, get answers that understand your specific context
- **Remembers Conversations** — References past discussions naturally ("I remember we discussed Sarah's performance in March...")
- **Protects Sensitive Data** — Auto-redacts SSNs and financial data before anything leaves your machine
- **Works Offline** — Browse employees and past conversations even without internet

## Who It's For

| Persona | Pain Point |
|---------|------------|
| Founder without HR | Wants to do right by people, no time to learn HR |
| Accidental HR person | Got the job by default, figuring it out as they go |
| Solo HR hero | 1 person, 200 employees, needs leverage not headcount |

## Tech Stack

- **Framework:** [Tauri](https://tauri.app/) — 5MB bundle, native performance
- **Frontend:** React + Vite + TypeScript + Tailwind CSS
- **Backend:** Rust with SQLite (local database)
- **AI:** Anthropic Claude API
- **Platform:** macOS (V1)

## Project Status

| Phase | Status | Description |
|-------|--------|-------------|
| 0. Pre-flight | ✅ Done | Tooling verified |
| 1. Foundation | ✅ Done | App runs, Claude API streaming, network detection |
| 2. Context | ✅ Done | AI knows your company, query-adaptive retrieval, 63 tests |
| 3. Protection | ✅ Done | PII redaction, audit logging, error handling, offline mode |
| 4. Polish | ✅ Done | Onboarding, Settings, Backup/Restore, Monday Digest |
| V2 Features | ✅ Done | Intelligence & visualization (analytics, signals, DEI, data quality) |
| 5. Launch | ✅ Done | Distribution, trial, license + seat limits, payment, landing page |

**Current:** Launch hardening complete. Remaining: Switch Stripe to live mode (5.5.5), E2E verification, then public launch.

## Key Features (Planned)

- [x] Architecture designed
- [x] Decisions locked (18 architectural decisions)
- [x] Chat interface with streaming responses
- [x] Employee CSV/Excel import with merge support
- [x] Company profile with state-specific context
- [x] Alex HR persona with employee/company awareness
- [x] Conversation sidebar with search
- [x] Cross-conversation memory
- [x] Smart prompt suggestions
- [x] PII auto-redaction with notification
- [x] Audit logging for compliance
- [x] Graceful error handling with retry
- [x] Read-only offline mode
- [x] Monday digest (anniversaries, new hires)
- [x] Encrypted data backup/restore
- [ ] App signing and notarization
- [ ] Auto-updates via GitHub Releases

## Development

```bash
# Start a development session
./scripts/dev-init.sh

# After Phase 1 scaffolding:
npm run dev        # Start dev server
npm run build      # Production build
cargo tauri dev    # Run Tauri app
```

## Documentation

| Document | Purpose |
|----------|---------|
| `CLAUDE.md` | Instructions for Claude Code sessions |
| `ROADMAP.md` | Implementation checklist |
| `docs/HR-Command-Center-Roadmap.md` | Full product roadmap |
| `docs/HR-Command-Center-Design-Architecture.md` | Technical specification |
| `PROGRESS.md` | Session-by-session log |
| `KNOWN_ISSUES.md` | Blockers and decisions |
| `docs/SESSION_PROTOCOL.md` | Multi-session workflow |

## Business Model

- **Price:** $99 one-time purchase
- **Includes:** App + lifetime updates
- **Not included:** AI API costs (~$2-8/month, paid to Anthropic)
- **No:** Subscriptions, per-seat fees, enterprise tiers

## Privacy

- All data stored locally in SQLite
- API keys stored in macOS Keychain
- PII auto-redacted before sending to AI
- Audit log of all AI interactions
- No telemetry without explicit opt-in

---

## License

Proprietary. See LICENSE file for details.

---

*Last updated: February 2026*
*Status: V2.4 — DEI & Fairness Lens complete, preparing for V2.5/Launch*
