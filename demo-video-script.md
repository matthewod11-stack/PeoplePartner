# People Partner — Demo Video Script

**Duration:** ~75 seconds
**Tone:** Calm, confident, unhurried. No stock music crescendos. Soft ambient piano or nothing.
**Resolution:** 1920×1080, app captured at native macOS resolution.

---

### SCENE 1 — The Problem
**Time:** 0:00–0:08
**Screen:** Black screen, then fade in a single line of white text on dark background:
> *"Your HR data lives in spreadsheets. Your questions live in your head."*

Fade to second line:
> *"What if they could talk to each other?"*

**Voiceover:**
> "You've got employee data in spreadsheets. Performance reviews in folders. And HR questions that take an hour to answer. What if all of that just... worked together?"

---

### SCENE 2 — First Launch & Import
**Time:** 0:08–0:25
**Screen:** App opens to the onboarding wizard. Show:
1. The welcome step — clean white card, teal accent, "Welcome to People Partner" with the three value-prop cards
2. Quick cut → Company Setup step: type "Oakwood Design" in the company name field, select "California" from the state dropdown
3. Quick cut → Employee Import step: drag a CSV file onto the FileDropzone (the dashed border turns teal), the column mapping table appears with green confidence badges auto-matching "Full Name," "Email," "Department"
4. Cut → Import complete screen: green checkmark, "47 employees processed — 47 created"
5. Cut → "You're all set!" step with the "Meet Alex" card visible at the bottom

**Voiceover:**
> "Setup takes two minutes. Name your company. Drop in a CSV — People Partner recognizes exports from BambooHR, Gusto, and Rippling automatically — maps the columns, validates the data, and you're in."

---

### SCENE 3 — The Main Interface
**Time:** 0:25–0:32
**Screen:** The full three-panel layout appears. Left sidebar shows the "People" tab with 47 employees listed, status filter tabs showing "42 Active · 3 Left · 2 Leave." The center chat area shows WelcomeContent with "What can I help with?" and four prompt suggestion pills. The right panel is empty with "Select an employee to view their details."

**Voiceover:**
> "This is your workspace. Employees on the left, your AI advisor in the center, and employee details on the right. Everything runs locally on your Mac — your data never leaves your machine."

---

### SCENE 4 — AI Advisor in Action
**Time:** 0:32–0:50
**Screen:** The cursor clicks in the chat input and types:
> *"Who on the engineering team has a declining performance trend?"*

Show the typing indicator (three bouncing dots), then the response streams in word by word. Alex responds with specific employee names, their rating history (e.g., "Marcus Chen: 4.2 → 3.8 → 3.1 over three cycles"), and a practical recommendation. A green "Verified" badge appears below the response. Click it to expand — showing each claim checked against the database with checkmarks.

Cut to a second question, already answered:
> *"What should I consider before putting Marcus on a PIP in California?"*

Alex's response references California-specific employment law, mentions documentation requirements, and suggests next steps.

**Voiceover:**
> "Ask a real question — People Partner pulls the right employee data automatically. No searching, no filtering. It knows who's on which team, their ratings, their reviews, their trajectory. And it verifies its own numbers against your actual data. Ask a follow-up, and it gives you state-specific guidance because it knows where your people work."

---

### SCENE 5 — Memory & PII Protection
**Time:** 0:50–1:05
**Screen:** Start a new conversation (click "New Conversation" in the sidebar). Type:
> *"Any update on the situation with Marcus?"*

Alex responds naturally — referencing the performance concern from the previous conversation without being told. Highlight the line where Alex says something like "Based on our earlier discussion about Marcus's declining performance..."

Quick cut: In the chat input, the user pastes text containing a Social Security number. A small amber notification slides in from the top: **"Redacted: 1 SSN"** — then fades after 3 seconds. The conversation continues uninterrupted.

**Voiceover:**
> "Start a new conversation days later, and People Partner remembers what you discussed. It picks up where you left off. And if sensitive data like a Social Security number slips into a message, it's automatically redacted before it ever reaches the AI. You see a quiet notification. That's it. Nothing leaves your device unprotected."

---

### SCENE 6 — The Close
**Time:** 1:05–1:15
**Screen:** Pull back to show the full app one more time — the three-panel layout, a conversation in progress, employees in the sidebar. Then a slow crossfade to a clean closing card:

> **People Partner**
> Your company's HR brain — private, always ready.
> **$99 one-time** · macOS
> peoplepartner.io

**Voiceover:**
> "People Partner. A private AI advisor that actually knows your team. Ninety-nine dollars, once. No subscription. No cloud. Just your data, on your Mac, working for you."

---

## Production Notes

| Element | Detail |
|---|---|
| **Total runtime** | ~75 seconds |
| **Scenes** | 6 (problem → import → interface → AI demo → memory+PII → close) |
| **Screen recordings needed** | 4 (onboarding flow, main interface, two chat interactions, PII redaction) |
| **Test data** | Use the 47-employee generated dataset; ensure "Marcus Chen" has a declining rating trend across 3 review cycles |
| **App state for recording** | Onboarding: fresh install. Main demo: post-import with conversations pre-seeded |
| **Captions** | Include — many viewers watch muted on Product Hunt |
| **Aspect ratio** | 16:9 primary, prepare a 1:1 crop of Scene 4 for social clips |
| **Don't show** | Settings panel, trial banner, upgrade flow, API key entry (these create friction in a first-impression video) |
| **Music** | Minimal ambient. Consider none — the voiceover and app sounds carry it. |
