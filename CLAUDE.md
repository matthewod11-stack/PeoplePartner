# People Partner — Claude Code Instructions

> **Vision:** Your company's HR brain—private, always in context, always ready to help.
> **Stack:** Tauri + React + Vite + SQLite + Claude API
> **Platform:** macOS only (V1)

---

## Quick Start

```bash
# Start every session with:
./scripts/dev-init.sh
```

This verifies the environment and shows current progress.

---

## Project Structure

```
peoplepartner/app/            # (this repo — desktop app)
├── README.md                 # Project overview & status (update on phase change)
├── CLAUDE.md                 # ← You are here
├── PROJECT_STATE.md          # Cross-surface sync doc
├── ROADMAP.md                # Task checklist with phases
├── AUDIT-2026-02-05.md       # Codebase audit report
├── features.json             # Pass/fail feature tracking
│
├── docs/
│   ├── PROGRESS.md           # Session log (last 5-10 sessions)
│   ├── HR-Command-Center-Roadmap.md      # Full product roadmap
│   ├── HR-Command-Center-Design-Architecture.md  # Technical specification
│   ├── SESSION_PROTOCOL.md   # How to run sessions
│   ├── KNOWN_ISSUES.md       # Blockers + locked decisions
│   ├── archive/              # Archived progress logs (Phases 0-2)
│   └── reference/            # Archive: feedback, decisions log
│
├── scripts/
│   ├── dev-init.sh           # Session initialization
│   └── generate-test-data.ts # Test data generator
│
├── src/                      # React frontend
│   ├── components/           # UI components (chat, employees, import, settings)
│   ├── contexts/             # React context providers
│   ├── hooks/                # Custom React hooks
│   └── lib/                  # Types, Tauri command wrappers
│
└── src-tauri/                # Rust backend
    ├── src/
    │   ├── lib.rs            # Tauri command exports
    │   ├── db.rs             # SQLite connection + migrations
    │   ├── chat.rs           # Claude API client + streaming
    │   ├── context.rs        # Context builder + Alex persona
    │   ├── employees.rs      # Employee CRUD
    │   └── ...               # Additional modules
    └── migrations/           # SQL migration files

# Sibling folders (see parent ~/Desktop/peoplepartner/CLAUDE.md):
# ../site/   — Marketing website (Next.js, Vercel)
# ../demo/   — Demo video factory (DaVinci Resolve)
# ../marketing/ — Ad copy, SEO audits, launch plans, brand materials
```

---

## Session Protocol

### This is a Multi-Session Project

Follow the **single-feature-per-session rule** to prevent scope creep.

### Before Working
1. Run `./scripts/dev-init.sh`
2. Read most recent entry in `PROGRESS.md` (historical entries in `docs/archive/`)
3. Check `ROADMAP.md` for next task
4. Check `KNOWN_ISSUES.md` for blockers

### After Each Task
1. Update `PROGRESS.md` (entry at TOP)
2. Update `features.json` status
3. Check off task in `ROADMAP.md`
4. Update `README.md` if phase status changed
5. Commit with descriptive message

### Progress Log Maintenance
When `PROGRESS.md` exceeds **10 sessions**, archive older entries:
1. Move entries beyond the last 5-7 to `docs/archive/PROGRESS_PHASES_X-Y.md`
2. Keep the file header and template comment
3. Update archive filename to reflect phases covered

**Why this works:** Archive files are never read at session start — only when tracing historical decisions. Their size doesn't affect context. Only `PROGRESS.md` needs to stay small.

### Session End Prompt
```
Before ending: Please follow session end protocol:
1. Run verification (build, type-check, tests)
2. Add session entry to TOP of PROGRESS.md
3. Update features.json with pass/fail status
4. Check off completed task in ROADMAP.md
5. Update README.md project status table if phase changed
6. If PROGRESS.md > 10 sessions, archive older entries
7. Commit with descriptive message

What's the "Next Session Should" note for PROGRESS.md?
```

---

## Current Phase

**Phase:** V2 Feature Planning Pause
**Status:** Reviewing deferred features before launch

**Completed Phases:**
- Phase 1 (Foundation) ✓
- Phase 2 (Context) ✓ — Pause Point 2A verified
- Phase 3 (Protection) ✓ — Pause Point 3A verified
- Phase 4 (Polish) ✓ — Pause Point 4A verified

**Current Focus:**
Review `KNOWN_ISSUES.md` to decide which V2 features to implement before Phase 5 (Launch).

**Candidate V2 Features:**
| Feature | Value | Complexity |
|---------|-------|------------|
| Interactive Analytics Panel | High | High |
| Org Chart View | High | Medium |
| Beginner-Friendly API Key Guide | High | Low |
| Persona Switcher | Medium | Low |
| Keyboard Shortcuts | Low | Low |

---

## Key Decisions (Do NOT Revisit)

| Area | Decision |
|------|----------|
| DB Security | OS sandbox only (no encryption at rest) |
| Context | Auto-include relevant employees |
| PII | Auto-redact and notify (no blocking modal) |
| PII Scope | Financial only (SSN, CC, bank) |
| Platform | macOS only |
| Offline | Read-only mode |
| Memory | Cross-conversation |
| Pricing | $99 one-time |

Full list in `KNOWN_ISSUES.md` under "Locked Architectural Decisions"

---

## Architecture Summary

### Data Flow
```
User Input → PII Scan → Context Builder → Memory Lookup → Claude API → Audit Log → Response
```

### Key Components
- **Frontend (React):** Chat UI, employee panels, import wizards, settings
- **Backend (Rust):** SQLite, context builder, Claude API client, Keychain

### Database Tables (9)
- `employees` - Core employee data with work_state, demographics, termination
- `conversations` - Chat history with title, summary
- `company` - Required: name + state
- `settings` - Key-value app config
- `audit_log` - Redacted request/response log
- `review_cycles` - Performance review periods
- `performance_ratings` - Numeric ratings (1.0-5.0)
- `performance_reviews` - Text narratives with FTS
- `enps_responses` - Employee Net Promoter Score

### Key Modules (Rust)
| Module | Purpose | Tests |
|--------|---------|-------|
| `context.rs` | Query extraction, employee retrieval, Alex persona prompt | 25 |
| `chat.rs` | Claude API, streaming, conversation trimming | 8 |
| `memory.rs` | Cross-conversation memory, summary generation, hybrid search | 8 |
| `conversations.rs` | Conversation CRUD, FTS search, title generation | 7 |
| `employees.rs` | Employee CRUD | - |
| `company.rs` | Company profile CRUD | - |
| `settings.rs` | Key-value settings store | - |

---

## Code Style

### React/TypeScript
- Functional components with hooks
- React Context for state (no Redux/Zustand)
- Tailwind CSS with design tokens from architecture doc
- TypeScript strict mode

### Rust
- SQLx for database (raw SQL, no ORM)
- Tauri commands for frontend communication
- All sensitive operations in Rust (API keys, PII scanning)

---

## Testing Approach

- **Context Builder:** 25 unit tests (query extraction, token estimation)
- **Chat Module:** 8 unit tests (trimming, message handling)
- **PII Scanner:** Unit tests for regex patterns (Phase 3)
- **UI:** Manual verification at pause points
- **E2E:** Verify at each phase pause point

Run tests: `cargo test --manifest-path src-tauri/Cargo.toml`

---

## Reference Documents

| Document | When to Read |
|----------|--------------|
| `docs/HR-Command-Center-Roadmap.md` | For phase context |
| `docs/HR-Command-Center-Design-Architecture.md` | When implementing |
| `docs/reference/DECISIONS-LOG.md` | When questioning approach |
| `docs/HR_PERSONA.md` | Alex persona system prompt |

---

## Environment Setup

**IMPORTANT**: This is a Tauri app requiring both Node.js and Rust toolchains.

### Quick Start
```bash
./scripts/dev-init.sh    # Verifies environment and shows current progress
```

### Manual Setup
```bash
# 1. Install Node dependencies
npm install

# 2. Verify Rust toolchain
rustc --version          # Should be 1.70+
cargo --version

# 3. Install Tauri CLI (if not present)
cargo install tauri-cli
```

### Environment Verification
```bash
# Check everything is ready
npm run type-check && cargo check --manifest-path src-tauri/Cargo.toml
```

---

## Testing Conventions

### Rust Tests (Backend)
```bash
# All tests
cargo test --manifest-path src-tauri/Cargo.toml

# Specific module
cargo test --manifest-path src-tauri/Cargo.toml context
cargo test --manifest-path src-tauri/Cargo.toml chat
cargo test --manifest-path src-tauri/Cargo.toml memory
```

### TypeScript (Frontend)
```bash
npm run type-check       # tsc --noEmit
```

### Key Test Modules
| Module | Location | Test Count |
|--------|----------|------------|
| `context.rs` | `src-tauri/src/context.rs` | 25 tests |
| `chat.rs` | `src-tauri/src/chat.rs` | 8 tests |
| `memory.rs` | `src-tauri/src/memory.rs` | 8 tests |
| `conversations.rs` | `src-tauri/src/conversations.rs` | 7 tests |

### Before Writing Tests
Always check existing test patterns in the module. Rust tests use `#[cfg(test)]` modules at the bottom of each file.

---

## Key Type Definitions

### Frontend Types (`src/lib/types.ts`)
| Type | Purpose |
|------|---------|
| `Employee` | Core employee record with work_state, demographics |
| `Conversation` | Chat session with title, messages |
| `Message` | Individual chat message (user/assistant) |
| `MemoryEntry` | Cross-conversation memory item |

### Tauri Commands (`src/lib/tauri-commands.ts`)
All frontend-backend communication goes through Tauri commands. Check this file for:
- Command names and signatures
- Request/response types
- Error handling patterns

### Rust Types
Key structs are in `src-tauri/src/`:
- `db.rs` — Database connection and migrations
- `employees.rs` — Employee model and CRUD
- `context.rs` — ContextBuilder, QueryIntent
- `chat.rs` — ChatMessage, ConversationTrimmer

---

## Common Commands

```bash
# Session start
./scripts/dev-init.sh

# Development
npm run dev           # Start Vite dev server
npm run build         # Production build
npm run type-check    # TypeScript check
cargo tauri dev       # Run Tauri app
cargo tauri build     # Build for distribution

# Testing
cargo test --manifest-path src-tauri/Cargo.toml           # All Rust tests
cargo test --manifest-path src-tauri/Cargo.toml context   # Context tests
cargo test --manifest-path src-tauri/Cargo.toml chat      # Chat tests

# Test data
npm run generate-test-data     # Generate test employees
npm run import-test-data       # Import to SQLite
```

---

## Commit Message Format

```
[Phase X.Y] Brief description

- Detail 1
- Detail 2

Session: YYYY-MM-DD
```

---

## PROJECT_STATE.md Maintenance

A `PROJECT_STATE.md` file exists at the project root. It serves as a cross-surface context sync document shared across Claude Chat, Claude Code (multiple machines), and Cowork.

**Rules:**
- Update `PROJECT_STATE.md` at the end of any session where meaningful changes were made
- Keep it under 300 lines — replace stale content, don't append indefinitely
- The "Recent Decisions" section should retain only the last 10 decisions; older ones can be archived or dropped
- The "Current State" section must reflect what actually exists in the codebase, not what was planned
- The "Cross-Surface Notes" section should flag any divergences from plans discussed outside this codebase
- When I say "update project state" or "sync state," regenerate `PROJECT_STATE.md` by scanning the current codebase
- Treat this file as the single source of truth about the project for external Claude sessions

---

*Last updated: March 2026*
*Status: All phases complete — pre-launch (targeting April 1, 2026)*
*Location: ~/Desktop/peoplepartner/app/*
