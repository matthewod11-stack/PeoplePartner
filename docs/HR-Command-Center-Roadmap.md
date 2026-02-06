# HR Command Center — Product Roadmap

> **Note:** This is the original product specification from planning (December 2025).
> For current implementation status, see [ROADMAP.md](../ROADMAP.md).

> **Vision:** Your company's HR brain—private, always in context, always ready to help.

---

## The Premise

HR professionals and founders shouldn't need to re-explain their company every time they ask an AI for help. They shouldn't worry about accidentally pasting SSNs into ChatGPT. They just want to ask a question and get an answer that *knows* their situation.

**Core Promise:** Open → Chat → Get help that understands your company.

---

## Target Users

| Persona | Pain Point |
|---------|------------|
| Founder without HR | Wants to do right by people, no time to learn HR |
| Accidental HR person | Got the job by default, figuring it out as they go |
| Solo HR hero | 1 person, 200 employees, needs leverage not headcount |
| Small business owner | Wants simple answers without enterprise pricing |
| Non-technical HR pro | Just wants to ask questions in plain English |

**What they have in common:** They need help, not software.

---

## What Makes This Different

| Generic AI (ChatGPT) | HR Command Center |
|---------------------|-------------------|
| "Tell me about your company..." every time | Already knows your employees, policies, context |
| Copy-paste data into chat | Query your local data naturally |
| Hope you didn't include SSNs | PII auto-redacted before anything leaves your machine |
| Generic advice | "Sarah is in California and has been here 3 years, so..." |
| Forgets everything between sessions | Remembers past conversations and context |

**The key:** Data stays local. Context is persistent. Privacy is built-in.

---

## Technical Foundation

**Stack:** Tauri + React + SQLite

| Component | Choice | Why |
|-----------|--------|-----|
| Framework | Tauri | 5MB bundle, native SQLite, secure by default |
| Frontend | React + Vite | Simple, fast, familiar |
| Database | SQLite | Local, no cloud dependency, your data stays yours |
| AI | Anthropic Claude | Best for nuanced HR conversations |
| Platform | macOS only (V1) | Focus on polish, native Keychain |

---

## Key Decisions (V1)

| Area | Decision | Rationale |
|------|----------|-----------|
| DB Security | OS sandbox only | Trust macOS security, simpler stack |
| Context | Auto-include relevant employees | Smart retrieval, no confirmation friction |
| PII | Auto-redact and notify | No blocking modals, brief notification |
| PII Scope | Financial only (SSN, CC, bank) | Narrow scope, fewer false positives |
| Offline | Read-only mode | Browse history + employees when offline |
| Memory | Cross-conversation | Compounding value over time |
| Pricing | $99 one-time | Simple, honest, no subscriptions |

---

## Phase 1: Foundation (Week 1-2)

**Goal:** App opens, stores data locally, talks to Claude.

### Tasks
- [x] Scaffold Tauri + React + Vite project
- [x] Create SQLite schema (employees, conversations, settings, audit_log)
- [x] Build basic chat UI (input, messages, scroll)
- [ ] Implement Claude API integration with streaming
- [ ] Store API key securely (macOS Keychain)
- [ ] API key validation on entry (test call, show Valid ✓ or Invalid ✗)
- [ ] Basic employee data model with work_state field
- [ ] Network connectivity detection

### Done When
You can open the app, paste an API key (validated immediately), and have a conversation with Claude.

---

## Phase 2: Context (Week 3-4)

**Goal:** Claude knows about your company and remembers past conversations.

### Tasks

**Data Import & Management**
- [ ] CSV import for employees (drag-drop)
- [ ] CSV re-import with merge logic (match by email, update changed fields)
- [ ] Individual employee add/edit UI
- [ ] Employee status management (active/terminated/leave)
- [ ] Sample dataset ("Acme Corp" with 5 employees) for demo/testing

**Context Intelligence**
- [ ] Smart context builder (auto-retrieve relevant employees based on query)
- [ ] Company profile setup (required: name + state)
- [ ] State/jurisdiction injection into AI context
- [ ] Cross-conversation memory (store summaries, reference past discussions)

**Conversation Management**
- [ ] Conversation sidebar (collapsible, left side)
- [ ] Auto-title conversations from first message
- [ ] Full-text search across conversations
- [ ] "New conversation" action

**Stickiness Features**
- [ ] Smart prompt suggestions (contextual, shown when input empty)
- [ ] Empty state guidance ("Start by importing your employee roster")

### Done When
You can ask "Who's been here longest?" and get the right answer. You can ask "What did we discuss about Sarah last month?" and Claude knows.

---

## Phase 3: Protection (Week 5)

**Goal:** Users can't accidentally leak sensitive data.

### Tasks

**PII Protection**
- [ ] PII scanner (SSN, credit cards, bank accounts)
- [ ] Auto-redact detected PII with placeholders (e.g., `[SSN_REDACTED]`)
- [ ] Brief notification when PII redacted ("SSN redacted for your protection")
- [ ] Audit log of what was sent to AI (with redacted content)

**Error Handling**
- [ ] Graceful error states (API down, rate limits, invalid key)
- [ ] User-friendly error messages (not raw API errors)
- [ ] Read-only offline mode (browse employees + past conversations)
- [ ] "Retry" and "Copy Message" actions on failure

### Done When
Pasting an SSN into chat auto-redacts it before sending, shows brief notification. App gracefully handles offline/error states.

---

## Phase 4: Polish (Week 6-7)

**Goal:** Feels like a real product.

### Tasks

**Onboarding**
- [ ] Welcome screen with value proposition
- [ ] API key setup (with "Get an API key" link to Anthropic)
- [ ] Company profile (required: name + state)
- [ ] Employee import (optional, can use sample data)
- [ ] Legal disclaimer acknowledgment ("This is not legal advice")
- [ ] Opt-in anonymous telemetry choice
- [ ] First conversation with pre-filled prompt suggestion

**Settings & Data**
- [ ] Settings panel (API key, data management)
- [ ] Data export (encrypted backup of all data)
- [ ] Data import (restore from backup)
- [ ] "Your data" indicator (shows SQLite file location)

**Engagement**
- [ ] Monday digest (anniversaries, new hire check-ins, proactive suggestions)

**Distribution**
- [ ] App icon and branding
- [ ] macOS code signing and notarization
- [ ] Auto-updates via GitHub Releases (tauri-plugin-updater)

### Done When
A non-technical user can download, install, and be productive within 2 minutes.

---

## Phase 5: Launch (Week 8)

**Goal:** Real users, real feedback.

### Tasks
- [ ] Landing page updates (hrcommandcenter.com)
- [ ] Payment integration (Stripe, $99 one-time)
- [ ] License key generation system
- [ ] One-time online license validation endpoint
- [ ] Beta distribution to 5-10 users
- [ ] Feedback collection (in-app feedback button)
- [ ] Iteration based on feedback

### Done When
Someone pays $99 and successfully uses the product.

---

## What We're NOT Building (V1)

| Feature | Why Not | V2 Consideration |
|---------|---------|------------------|
| Document/PDF ingestion | Focus on employee context first | Based on user demand |
| Multi-company support | 90% of users need single company | If consultants request |
| Multi-state per employee | Single location covers 80% | Based on user feedback |
| Keyboard shortcuts | Nice but not essential | V1.1 quick add |
| Windows/Linux | Focus on macOS polish | If market demands |
| Dark mode | Ship light theme first | Easy to add later |
| HIPAA/medical PII detection | Narrow scope reduces false positives | If users request |

These can come later. V1 is about the core promise.

---

## Success Metrics

| Metric | Target |
|--------|--------|
| Time to first conversation | < 2 minutes |
| App bundle size | < 15MB |
| Chat response time | < 3 seconds |
| Setup steps | 4 (API key → Company → Import → Chat) |
| Lines of code | < 3,500 |
| Day-7 retention | > 40% |

---

## The Jobs Test

Before adding ANY feature, ask:

1. What would this look like if I started from zero?
2. Where am I adding complexity users don't value?
3. What would this be like if it just worked magically?
4. Am I including this because I can, or because I should?
5. Does this make it feel inevitable or complicated?

If it fails the test, don't build it.

---

## Business Model

- **Price:** $99 one-time
- **What's included:** App + lifetime updates
- **What's not:** AI API costs (~$2-8/month, user pays Anthropic directly)
- **No:** Subscriptions, per-seat fees, enterprise tiers, upsells
- **License:** One-time online validation, works offline forever after

Keep it simple. Keep it honest.

---

## Timeline Summary

| Phase | Duration | Outcome |
|-------|----------|---------|
| 1. Foundation | 2 weeks | App works, API validated |
| 2. Context | 2 weeks | AI knows your company, remembers conversations |
| 3. Protection | 1 week | PII auto-redacted, graceful errors |
| 4. Polish | 2 weeks | Onboarding, export, digest |
| 5. Launch | 1 week | Real users, real feedback |

**Total: ~8 weeks to v1.0**

---

## Guiding Principles

1. **Chat-first, always.** No menus, no dashboards, no "HR software" vibes.
2. **Private by design.** Data never leaves the machine unless explicitly sent to AI.
3. **Respect the user.** They're smart, busy, and not engineers. Don't make them feel dumb.
4. **Simple > Feature-rich.** Do one thing brilliantly before adding a second thing.
5. **Ship, then iterate.** A working simple product beats a planned complex one.
6. **Trust over friction.** Auto-include, auto-redact, auto-remember. Don't ask permission for every action.

---

## V1.1 Backlog (Post-Launch)

Based on feedback and deferred decisions:

- [ ] Keyboard shortcuts (Cmd+N, Cmd+K, etc.)
- [ ] Multi-state employee locations
- [ ] Single document ingestion (handbook PDF)
- [ ] Windows support
- [ ] Saved snippets / templates
- [ ] Progress indicators ("47 questions this quarter")

---

*Last updated: December 2025*
*Decisions locked. Ready to build.*
