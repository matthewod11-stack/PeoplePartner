# HR Command Center — Decision Log

> **Date:** December 12, 2025
> **Purpose:** Architectural and strategic decisions that will inform Roadmap and Architecture updates

---

## Summary Table

| Category | Decision | Implication |
|----------|----------|-------------|
| DB Security | OS Sandbox Only | No SQLCipher, simpler stack |
| Context Default | Auto-include relevant employees | Smart retrieval, no confirmation modal |
| PII Action | Auto-redact and notify | No blocking modal, brief notification |
| PII Scope | Financial only (SSN, CC, bank) | No medical/immigration detection in V1 |
| Platforms | macOS only | Single target, native Keychain |
| Pricing | $99 one-time | No subscription, no tiers |
| Offline Mode | Read-only | Browse history + employees, no new questions |
| Work Locations | Single location (defer multi to V2) | Simple jurisdiction logic |
| Company Profile | Required: name + state only | Minimal friction, ensures context |
| Doc Ingestion | Not in V1 | Focus on employee data context |
| Multi-Company | Single company only | Simple data model |
| Audit Log | Standard (redacted content) | Balance of compliance + privacy |
| Disclaimers | Onboarding acknowledgment only | One-time acceptance, clean chat UI |
| Crash Reports | Opt-in anonymous telemetry | Onboarding choice, helps improvement |
| Stickiness V1 | Sidebar + prompts + digest (no shortcuts) | 3 of 4 high-consensus features |
| Data Updates | CSV re-import + individual edit | Bulk and quick-fix both supported |
| Memory | Cross-conversation | Compounding value, summarization needed |
| License | One-time online validation | No ongoing server checks |

---

## Detailed Decisions

### 1. Database Security
**Decision:** OS Sandbox Only (no encryption at rest)

**Rationale:** Trust macOS user account security. Simpler implementation, easier debugging, no key management complexity.

**Trade-off accepted:** Anyone with device access can read SQLite data directly.

---

### 2. Context Injection Default
**Decision:** Auto-include relevant employees

**Rationale:** Smart retrieval when user asks about specific employees. Feels magical, reduces friction.

**Implementation note:** Build semantic matching to find relevant employee records based on query content.

---

### 3. PII Detection & Action
**Decision:** Auto-redact and notify

**Rationale:** Least friction. User sees brief notification "SSN redacted for your protection" rather than blocking modal.

**Implementation note:** Replace detected PII with placeholders like `[SSN_REDACTED]` before API call.

---

### 4. PII Scope
**Decision:** Financial PII only (SSN, credit cards, bank accounts)

**Rationale:** Narrow scope reduces false positives. Medical/immigration detection adds complexity without proven user demand.

**V2 consideration:** Expand scope based on user feedback about what they accidentally paste.

---

### 5. Platform Support
**Decision:** macOS only for V1

**Rationale:** Focus on polish for one platform. Target audience (founders, small business) skews Mac. Uses native Keychain.

**V2 consideration:** Windows support if market demand proves significant.

---

### 6. Pricing Model
**Decision:** $99 one-time (as originally planned)

**Rationale:** Simple, honest, no subscription fatigue. Aligns with "no-nonsense" brand positioning.

**Risk accepted:** No recurring revenue; relies on volume.

---

### 7. Offline Behavior
**Decision:** Read-only offline mode

**Rationale:** App still provides value (browse employees, past conversations) even without internet. New questions disabled with friendly message.

**Implementation note:** Check network before API call; if offline, disable input with "Offline - browse your data or try again when connected."

---

### 8. Employee Work Locations
**Decision:** Single primary location per employee (V1)

**Rationale:** Covers 80% of cases. Multi-state complexity deferred to V2 based on user demand.

**Schema impact:** `work_state` field (single value), not array.

---

### 9. Company Profile Requirements
**Decision:** Require company name + state only

**Rationale:** Minimal friction (2 fields) ensures every AI response has jurisdiction context.

**Onboarding impact:** Step 3 is required but fast: "What's your company name?" + state dropdown.

---

### 10. Document Ingestion
**Decision:** Not in V1

**Rationale:** Focus on employee data context as core differentiator. Document RAG adds chunking/embedding complexity.

**V2 consideration:** Start with single-document support if users frequently request it.

---

### 11. Multi-Company Support
**Decision:** Single company only (V1)

**Rationale:** Covers 90% of target users. HR consultants can use separate app instances.

**Schema impact:** No `company_id` foreign keys needed.

---

### 12. Audit Log Depth
**Decision:** Standard - redacted content

**Rationale:** Balances compliance needs with privacy. Questions and responses logged with PII replaced by placeholders.

**Export capability:** Users can export audit log for legal holds.

---

### 13. Legal Disclaimers
**Decision:** Onboarding acknowledgment only (V1); Feature-specific consent modals (V2)

**Rationale:** One-time acceptance during first launch. Keeps chat UI clean without constant reminders.

**Implementation:** Checkbox + "I understand this is not legal advice and I should verify with qualified counsel."

**V2 Evolution (February 2026):** V2 features (Attention Signals, DEI & Fairness Lens) introduce feature-specific first-use consent modals. This is an intentional evolution — the original decision referred to general chat disclaimers, not specialized feature onboarding. These modals explain the limitations and appropriate use of predictive/analytical features before first use, then don't appear again.

---

### 14. Crash Reporting
**Decision:** Opt-in anonymous telemetry

**Rationale:** Respect user choice while enabling product improvement. Ask during onboarding.

**Implementation:** Use Sentry or similar with user consent flag. Anonymize before sending.

---

### 15. V1 Stickiness Features
**Decision:** Include 3 of 4 high-consensus features

| Feature | Included | LOC Estimate |
|---------|----------|--------------|
| Conversation sidebar + search | ✅ Yes | ~150 |
| Smart prompt suggestions | ✅ Yes | ~100 |
| Monday digest / notifications | ✅ Yes | ~150 |
| Keyboard shortcuts | ❌ V1.1 | ~50 |

**Total stickiness LOC:** ~400

---

### 16. Employee Data Updates
**Decision:** Both CSV re-import AND individual editing

**Rationale:** Covers bulk updates (re-import from HRIS export) and quick fixes (typo correction, promotion).

**Implementation notes:**
- CSV re-import matches by email, updates changed fields
- Individual edit via click → form modal
- ~100 LOC for edit UI

---

### 17. Cross-Conversation Memory
**Decision:** Yes - Claude references past conversations

**Rationale:** Creates compounding value. Longer usage = smarter assistant = harder to switch away.

**Implementation notes:**
- Store conversation summaries (auto-generated)
- Include relevant past summaries in context when question relates to previously discussed topics
- ~100 LOC for summarization + retrieval

---

### 18. License Validation
**Decision:** One-time online validation

**Rationale:** User enters key, validates once against server, works offline forever after.

**Implementation notes:**
- Simple API endpoint: POST /validate with license key
- Store validation result locally
- No ongoing server dependency

**Risk accepted:** Keys can be shared (no device limit), but target market unlikely to pirate.

---

## Impact on Documents

### Roadmap Updates Needed
- Phase 2: Add conversation sidebar, smart prompts, cross-conversation memory
- Phase 3: Add auto-redact notification UX
- Phase 4: Add Monday digest, opt-in telemetry prompt, license validation
- Phase 4: Add CSV re-import merge logic + individual employee edit UI

### Architecture Updates Needed
- Add Context Builder spec (auto-retrieval logic)
- Add Audit Log schema and redaction approach
- Add Offline Mode section
- Add License Validation flow
- Add Telemetry section (opt-in, anonymous)
- Update Employee schema (confirm single work_state)
- Add Conversation Summary storage for memory feature

---

*Decision log created: December 12, 2025*
