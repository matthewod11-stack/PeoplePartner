# HR Command Center — Session Protocol

> **Purpose:** Ensure continuity across multiple Claude Code sessions.
> **Based on:** [Anthropic: Effective Harnesses for Long-Running Agents](https://www.anthropic.com/engineering/effective-harnesses-for-long-running-agents)

---

## Core Principle

> "Each new session begins with no memory of what came before."

## Current Implementation Notes (Phase 5)

- Proxy request signing (`TRIAL_SIGNING_SECRET` / `HRCOMMAND_PROXY_SIGNING_SECRET`) remains **optional for local/dev** workflows.
- Release placeholders (updater pubkey/repo endpoint, Cloudflare KV IDs, purchase URLs) are **intentionally deferred** until release configuration.

We use **structured artifacts** to maintain continuity:

| Artifact | Purpose | Location |
|----------|---------|----------|
| **README.md** | Project overview & status | `README.md` |
| **PROGRESS.md** | Log of completed work | `docs/PROGRESS.md` |
| **ROADMAP.md** | Checkbox tracking | `ROADMAP.md` |
| **features.json** | Pass/fail status | `features.json` |
| **KNOWN_ISSUES.md** | Parking lot | `docs/KNOWN_ISSUES.md` |
| **DECISIONS-LOG.md** | Architectural decisions | `docs/reference/DECISIONS-LOG.md` |

---

## Quick Reference Card

```
╔═══════════════════════════════════════════════════════════════════════╗
║  HR COMMAND CENTER - SESSION MANAGEMENT                               ║
╠═══════════════════════════════════════════════════════════════════════╣
║                                                                       ║
║  SESSION START:                                                       ║
║    ./scripts/dev-init.sh                                              ║
║                                                                       ║
║  DURING SESSION:                                                      ║
║    • Work on ONE task at a time                                       ║
║    • Update docs after each completed task                            ║
║    • Commit frequently with descriptive messages                      ║
║                                                                       ║
║  CHECKPOINT (context getting long):                                   ║
║    "Update PROGRESS.md and features.json with current state"          ║
║                                                                       ║
║  SESSION END (before compaction):                                     ║
║    "Before ending: Please follow session end protocol..."             ║
║                                                                       ║
║  IF BLOCKED:                                                          ║
║    Add to KNOWN_ISSUES.md → Move to next independent task             ║
║                                                                       ║
╚═══════════════════════════════════════════════════════════════════════╝
```

---

## Session Start Protocol

1. **Run init script:** `./scripts/dev-init.sh`
2. **Read progress:** Review `docs/PROGRESS.md` for previous session work
3. **Check features:** Review `features.json` for pass/fail status
4. **Verify previous work:** Run `npm run build` and tests
5. **Check blockers:** Review `docs/KNOWN_ISSUES.md`
6. **Pick next task:** First unchecked item in `ROADMAP.md`

### Session Start Prompt

```
I'm continuing work on HR Command Center.

This is a multi-session implementation. Please follow the session protocol:

1. Run ./scripts/dev-init.sh to verify environment
2. Read docs/PROGRESS.md for previous session work
3. Read ROADMAP.md to find the NEXT unchecked task
4. Check features.json for pass/fail status
5. Check docs/KNOWN_ISSUES.md for any blockers

Work on ONE task only (single-feature-per-session rule). Tell me what's next.
```

---

## Session End Protocol

1. Run verification (build, type-check, tests)
2. Add entry to TOP of `docs/PROGRESS.md`
3. Update `features.json` status
4. Check off tasks in `ROADMAP.md`
5. Update `README.md` project status if phase changed or major milestone reached
6. Commit with descriptive message
7. Note "Next Session Should" in PROGRESS.md

### Session End Prompt

```
Before ending: Please follow session end protocol:

1. Run verification (build, type-check, tests)
2. Add session entry to TOP of docs/PROGRESS.md
3. Update features.json with pass/fail status
4. Check off completed task in ROADMAP.md
5. Update README.md project status table if phase changed
6. Commit with descriptive message

What's the "Next Session Should" note for PROGRESS.md?
```

---

## Checkpoint Prompt (Mid-Session)

Use when context is getting long or before a complex operation:

```
Let's checkpoint. Update docs/PROGRESS.md and features.json
with current state, then we can continue.
```

---

## After Long Break Prompt

Use when resuming after days/weeks away:

```
Resuming HR Command Center after a break. Full context reload:

1. Run ./scripts/dev-init.sh
2. Read docs/SESSION_PROTOCOL.md (workflow rules)
3. Read docs/PROGRESS.md (all session history)
4. Read DECISIONS-LOG.md for architectural decisions
5. Check features.json and KNOWN_ISSUES.md

Summarize: where are we, what's next, any blockers?
```

---

## Key Project Documents

| Document | Purpose | When to Read |
|----------|---------|--------------|
| `docs/HR-Command-Center-Roadmap.md` | Full product roadmap | For context on phases |
| `docs/HR-Command-Center-Design-Architecture.md` | Technical spec | When implementing |
| `DECISIONS-LOG.md` | Why we made choices | When questioning approach |
| `MASTER-FEEDBACK-CONSOLIDATED.md` | AI feedback analysis | For feature prioritization |
| `ROADMAP.md` | Task checklist | Every session |
| `docs/PROGRESS.md` | Session log | Every session |
| `features.json` | Pass/fail tracking | Every session |

---

## Understanding Sessions vs Tasks

**Session = Context Window** (not calendar day, not task)

```
┌─────────────────────────────────────────────────────────────┐
│  Context Window                                             │
│                                                             │
│  Task A ──► Task B ──► Task C ──► [Context limit]           │
│    ↓          ↓                         ↓                   │
│  Update    Update              SESSION ENDS                 │
│   docs      docs               (update docs)                │
└─────────────────────────────────────────────────────────────┘
                              ↓
              ┌───────────────────────────────┐
              │  NEW SESSION                  │
              │  Run init, read progress      │
              │  Continue Task C or next      │
              └───────────────────────────────┘
```

- **Update docs** → After every completed task
- **New session** → After compaction or fresh start
- **Can complete multiple tasks** in one context window
- **Large tasks can span** multiple sessions

---

## Commit Message Format

Use descriptive commits that serve as documentation:

```
[Phase X.Y] Brief description

- Detail 1
- Detail 2

Session: YYYY-MM-DD
```

Examples:
```
[Phase 1.1] Scaffold Tauri + React + Vite project

- Initialized project with create-tauri-app
- Added Tailwind CSS with design tokens
- Configured TypeScript strict mode

Session: 2025-12-13
```

---

## Tips for Success

1. **Start sessions the same way** — Always run dev-init.sh
2. **Checkpoint often** — Don't wait for context limit
3. **PROGRESS.md entries at TOP** — Most recent first
4. **Descriptive commits** — They serve as documentation
5. **Park blockers immediately** — Don't let them derail progress
6. **Verify before marking complete** — Build/tests must pass
7. **Reference architecture doc** — When unsure about implementation details

---

*Protocol Version: 1.0*
*Project: HR Command Center*
