# People Partner — Website vs. Product Audit

**Date:** March 2, 2026
**Scope:** Compare peoplepartner.io website copy against HRCommand codebase reality

---

## Summary

The website undersells the product. It positions People Partner as a private AI chatbot for HR questions, which is accurate but incomplete. The tool has evolved well past "chat with context" into a full employee data platform with performance management, engagement tracking, team analytics, document ingestion, and compliance logging. None of that is on the site.

The copy itself is clean and readable. The persona sections are strong. But the feature list feels like it was written when the product was at Phase 1, and the codebase is now at Phase 4+. There's a meaningful gap between what someone would expect after reading the site and what they'd actually get.

---

## Section 1: What the Website Claims (6 Features)

| Feature on Site | Status in Codebase |
|---|---|
| Cross-conversation memory | ✅ Fully built — hybrid search (summary + FTS fallback), Claude-generated summaries |
| 5 specialist personas | ✅ Fully built — Alex, Jordan, Sam, Morgan, and a 5th. Selectable in settings. |
| Auto PII redaction | ✅ Fully built — SSN, CC, bank account regex scanning with redaction before API calls |
| 100% local storage | ✅ SQLite on disk, macOS Keychain for API keys |
| Import your roster (CSV/Excel) | ✅ Fully built — plus HRIS presets, column mapping UI, dedup detection, validation pipeline |
| Encrypted backups | ✅ AES-256-GCM with Argon2id key derivation, compressed |

Everything claimed on the site is real. No accuracy issues. The problem is omission, not misrepresentation.

---

## Section 2: What's Built but Missing from the Website

These are real, functioning features in the codebase that a visitor to peoplepartner.io would never know about.

### HIGH-VALUE GAPS (likely deal-closers for the target audience)

**1. Performance Review System**
Full review cycle management: create review periods, collect narrative reviews (strengths, areas for improvement, accomplishments, goals, manager comments, self-assessments), numeric ratings on a 1.0–5.0 scale, rating distributions, and full-text search across all review content.

*Why this matters:* Solo HR people and founders managing reviews in spreadsheets would pay for this alone. It turns People Partner from "a chatbot" into "where I manage performance."

**2. AI-Extracted Review Highlights**
Claude reads performance reviews and extracts structured quotes, key themes, and sentiment. Generates per-employee summaries from review history. Batch extraction pipeline for processing all reviews at once.

*Why this matters:* This is a genuine differentiator. No competitor at the $99 price point does AI-powered review synthesis. It's the kind of feature that makes someone think "wait, it does *that*?"

**3. eNPS Tracking**
Employee Net Promoter Score: per-employee scores (0–10), survey periods, feedback text, aggregate score calculation with promoter/passive/detractor breakdowns.

*Why this matters:* eNPS is one of the most commonly requested metrics from leadership. Having it built into the same tool where you chat about employees is a strong hook.

**4. Document Ingestion**
Points at a folder of HR documents (handbooks, policies, memos — supports .md, .txt, .csv, .pdf, .docx, .xlsx) and indexes them with PII redaction. Then includes relevant document chunks in AI context automatically.

*Why this matters:* This is the single biggest differentiator vs. "just use ChatGPT." The AI doesn't just know your employees — it knows your policies. For someone who's been pasting handbook sections into ChatGPT, this is transformative.

**5. Team Signals (Attrition Risk)**
Department-level risk indicators using tenure, performance, and engagement weights. Team-level only (never individual predictions), opt-in with disclaimers, minimum group sizes enforced for privacy.

*Why this matters:* Even in heuristic form, "early warning signals" is a headline feature. The ethical guardrails (team-only, disclaimers, opt-in) make it even more compelling for HR people who are nervous about AI overreach.

**6. DEI / Fairness Lens**
Demographic representation analysis across departments with privacy protection (groups under 5 suppressed), promotion inference from title changes, strong disclaimers. Opt-in with first-use acknowledgment.

*Why this matters:* DEI analysis is frequently requested but hard to do without enterprise tools. Having it with built-in privacy protections is a selling point for compliance-conscious buyers.

### MEDIUM-VALUE GAPS (feature-list strengtheners)

**7. Multi-Provider Support**
The FAQ briefly mentions supporting OpenAI, Anthropic, and Google. But the codebase has a full provider abstraction layer with separate implementations, key management per provider, and a provider picker UI. This deserves more than a FAQ footnote.

**8. Audit Log with CSV Export**
Every AI interaction is logged — what was asked, what was redacted, what was returned. Exportable to CSV. This is a compliance feature that HR professionals in regulated environments would care about.

**9. Monday Digest**
Weekly digest card showing work anniversaries this week and new hires in the last 90 days. Appears on first app launch of the week. Small touch but signals that the tool is proactively working for you.

**10. Command Palette**
VS Code/Slack-style ⌘K fuzzy search across actions, conversations, and employees. Power-user feature that signals polish.

**11. Smart Import Pipeline**
The website says "import your roster." The actual implementation includes: HRIS preset detection, intelligent column mapping with confidence scores, duplicate detection, validation with fix-and-retry, support for importing employees, ratings, reviews, and eNPS data. This is substantially more than "CSV import."

**12. Conversation Search**
Full-text search across all past conversations. Not mentioned on the site.

---

## Section 3: Website Copy Assessment

### What's Working

The **persona sections** (Founders, Solo HR, Accidental HR, Small Business) are the strongest part of the page. They read like someone who actually talks to these people. The "You built the product. Now you're Googling 'how to fire someone'" line is specific and real.

The **pricing section** is clean and honest. "$99 one-time" with the API cost callout is straightforward. No tricks.

The **security section** lands the core message: your data stays local, PII gets stripped.

### What's Not Working

**The feature grid is too thin.** Six features, most of them defensive (privacy, local, encrypted). The site reads like a security product that happens to do HR chat. The actual product is an HR platform that happens to be secure. The framing is backwards.

**No screenshots or product visuals.** The site is entirely text. For a $99 desktop app, people want to see what they're buying. A single screenshot of the chat interface with employee context would do more than three paragraphs of copy.

**"See how it works" links to nothing actionable.** If there's no demo video, this CTA is a dead end. Either build a short walkthrough or remove the link.

**The FAQ is hiding features.** Multi-provider support, the context system, the PII handling detail — these are buried in expandable FAQ items that most visitors won't read.

**No social proof.** No testimonials, no user count, no logos, no "built by an HR professional" narrative. For a solo product in a trust-heavy category (people's employee data), some kind of credibility signal is needed.

**The headline is generic.** "Your company's people brain" — could describe any HR tool. The subhead does better work ("AI-powered HR guidance that remembers your team...") but the hook needs to be sharper.

### Conversion Concerns

For the target audience (solo practitioners, founders, small-team HR), the site needs to answer three questions fast:

1. **What does this actually do?** — The site answers this too vaguely. "HR guidance" covers everything from answering a question to managing performance reviews. The product does both, but the site only communicates the first.

2. **Why not just use ChatGPT?** — The site gestures at this with "local storage" and "PII redaction" but never makes the affirmative case for what you *gain*, only what you avoid losing. The document ingestion, cross-conversation memory, and employee context system are the real answer to this question, and they're underplayed.

3. **Is this legit?** — No screenshots, no demo, no social proof. For a product that asks people to load their employee data, trust matters. The copy reads professional, but the page doesn't prove the product exists as a real, polished application.

---

## Section 4: Specific Recommendations

### Immediate (copy changes, no new features needed)

1. **Expand the feature grid from 6 to 10–12.** Add: performance reviews, eNPS, document ingestion, audit logging, review highlights, team signals. These are built and working.

2. **Add screenshots.** Three minimum: the chat interface with employee context visible, the employee detail view with performance data, and the import wizard. Even redacted/dummy-data screenshots are fine.

3. **Reframe the headline.** Instead of "Your company's people brain," try something that communicates the actual depth: "The HR platform that runs on your Mac" or "Performance reviews, employee data, and AI guidance — all local, all private." Test a few.

4. **Add a "Why not ChatGPT?" section.** Make it direct. Three concrete differences: (a) it knows your employees without re-explaining, (b) it reads your handbook and policies, (c) it never sends SSNs to the cloud. These are specific and defensible.

5. **Move multi-provider support out of the FAQ.** "Works with Claude, GPT-4, and Gemini" is a top-level feature, not a footnote.

6. **Kill or replace "See how it works."** Either link to a 90-second Loom walkthrough or replace with a screenshot carousel.

### Medium-term (requires some effort)

7. **Create a product tour or demo video.** 60–90 seconds. Show: import employees → ask a question → see context auto-included → PII gets caught. This single asset would likely double conversion.

8. **Add a "What's included" breakdown** that separates the product into clear categories: Employee Management, Performance, Engagement (eNPS), AI Chat, Privacy & Compliance, Data Management. This helps visitors understand this isn't just a chatbot.

9. **Collect and display 3–5 early user quotes.** Even informal ones. "I stopped pasting employee data into ChatGPT" is worth more than any feature description.

10. **Consider a comparison table** — People Partner vs. ChatGPT vs. BambooHR vs. "spreadsheet + Google." Position on privacy AND functionality, not just privacy.

---

## Section 5: Legal/Compliance Flags

| Issue | Severity | Note |
|---|---|---|
| "Lifetime updates" claim in pricing | Medium | Consider clarifying scope — does this mean all future versions? Major version upgrades? Bug fixes only? Ambiguous lifetime promises can create customer service headaches. |
| "We respond in minutes, not days" | Low | Sets an expectation that may be hard to maintain at scale. Consider "We respond quickly" or add business hours context. |
| No Terms of Service link visible in main nav | Medium | Footer has Privacy/Terms links, but they should be accessible and substantive before people purchase. |
| PII disclaimer wording | Low | "People Partner catches high-risk financial data before it reaches the AI" — accurate based on the codebase. The follow-up about "relevant details are included" is a good transparency note. |

---

## Bottom Line

The product has outgrown the website. You've built a legitimate HR data platform with performance management, engagement tracking, document ingestion, team analytics, and compliance logging. The site is still selling "a private AI chatbot."

The six features on the site are real, but they're maybe 40% of what the tool actually does. The missing 60% contains the features most likely to convert a visitor from "interesting" to "I need this."

Priority actions: expand the feature list, add screenshots, reframe the headline around depth (not just privacy), and build a short demo video.
