# HR Command Center — Implementation Roadmap

> **Purpose:** Actionable checklist for implementation across multiple sessions.
> **Related Docs:** [SESSION_PROTOCOL.md](./docs/SESSION_PROTOCOL.md) | [PROGRESS.md](./docs/PROGRESS.md)
> **Full Spec:** [HR-Command-Center-Roadmap.md](./docs/HR-Command-Center-Roadmap.md)

---

## Session Management

This is a **long-running, multi-session implementation**. Follow these rules:

### Before Each Session
```bash
./scripts/dev-init.sh
```

### Single-Feature-Per-Session Rule
> **CRITICAL:** Work on ONE checkbox item per session when possible. This prevents scope creep and ensures proper documentation.

### After Each Session
1. Run verification (build, type-check, tests)
2. Update PROGRESS.md with session entry
3. Update features.json status
4. Check off completed tasks below
5. Commit with descriptive message

---

## Phase Overview

| Phase | Focus | Status | Pause Points |
|-------|-------|--------|--------------|
| 0 | Pre-flight validation | ✓ Complete | 0A: Tooling verified ✓ |
| 1 | Foundation | ✓ Complete | 1A: App runs, API works ✓ |
| 2 | Context | ✓ Complete | 2A: Context injection works ✓ |
| 3 | Protection | ✓ Complete | 3A: PII redaction works ✓ |
| 4 | Polish | ✓ Complete | 4A: Onboarding complete ✓ |
| V2 | Intelligence & Visualization | ✓ Complete | V2.1-V2.5: See below |
| 5 | Launch | ✓ Complete | 5A ✓, 5B ✓, 5C ✓, 5.5.5 ✓ |

---

## Phase 0: Pre-Flight Validation

**Goal:** Confirm tooling is ready and environment is set up

### Tasks
- [x] 0.1 Verify Rust toolchain installed (`rustc --version`) ✓ 1.92.0
- [x] 0.2 Verify Node.js installed (`node --version`) ✓ v22.21.0
- [x] 0.3 Verify Tauri CLI installed (`cargo tauri --version`) ✓ 2.9.6
- [x] 0.4 Create empty Git repository with .gitignore
- [x] 0.5 Document environment versions in PROGRESS.md

### Pause Point 0A ✓ COMPLETE
**Action Required:** Confirm all tooling works before proceeding — **VERIFIED**

---

## Phase 1: Foundation

**Goal:** App opens, stores data locally, talks to Claude

### 1.1 Project Scaffolding ✓ COMPLETE
- [x] 1.1.1 Initialize Tauri + React + Vite project
- [x] 1.1.2 Configure TypeScript
- [x] 1.1.3 Set up Tailwind CSS with design tokens
- [x] 1.1.4 Create basic folder structure per architecture doc
- [x] 1.1.5 Verify `npm run dev` launches app window

### 1.2 SQLite Setup ✓ COMPLETE
- [x] 1.2.1 Add SQLx dependency to Cargo.toml
- [x] 1.2.2 Create initial migration (employees, conversations, company, settings, audit_log)
- [x] 1.2.3 Create FTS virtual table for conversation search
- [x] 1.2.4 Implement db.rs with connection management
- [x] 1.2.5 Verify database creates on first launch

### 1.3 Basic Chat UI ✓ COMPLETE
- [x] 1.3.1 Create AppShell component (main layout)
- [x] 1.3.2 Create ChatInput component
- [x] 1.3.3 Create MessageBubble component (user/assistant variants)
- [x] 1.3.4 Create MessageList component with scroll
- [x] 1.3.5 Create TypingIndicator component
- [x] 1.3.6 Wire up basic message send/display flow

### 1.4 Claude API Integration
- [x] 1.4.1 Add keyring dependency for macOS Keychain
- [x] 1.4.2 Implement keyring.rs for API key storage
- [x] 1.4.3 Create ApiKeyInput component with validation
- [x] 1.4.4 Implement chat.rs with Claude API call
- [x] 1.4.5 Add response streaming support
- [x] 1.4.6 Wire frontend to backend via Tauri invoke

### 1.5 Network Detection ✓ COMPLETE
- [x] 1.5.1 Implement network check in Rust
- [x] 1.5.2 Create useNetwork hook in React
- [x] 1.5.3 Show offline indicator when disconnected

### Pause Point 1A ✓ VERIFIED
**Verification Required:**
- [x] App window opens
- [x] Can enter API key (validates against Claude)
- [x] Can send message and receive streamed response
- [x] Messages persist in SQLite
- [x] Network status displays correctly

---

## Phase 2: Context

**Goal:** Claude knows about your company and remembers past conversations

### 2.1 Employee & Performance Data
> **Expanded Scope:** Full HR Suite schema with performance ratings, reviews, eNPS, and demographics.
> **Reference:** [SCHEMA_EXPANSION_V1.md](./SCHEMA_EXPANSION_V1.md)

#### 2.1.A Schema & Backend
- [x] 2.1.1 Create migration 002_performance_enps.sql (new tables + employee fields)
- [x] 2.1.2 Update employees.rs with demographics + termination fields
- [x] 2.1.3 Create review_cycles.rs CRUD operations
- [x] 2.1.4 Create performance_ratings.rs CRUD operations
- [x] 2.1.5 Create performance_reviews.rs CRUD operations (+ FTS triggers)
- [x] 2.1.6 Create enps.rs CRUD operations

#### 2.1.B File Ingestion
- [x] 2.1.7 Add calamine dependency for Excel parsing
- [x] 2.1.8 Create unified file parser (CSV, XLSX, TSV)
- [x] 2.1.9 Create FileDropzone component (multi-format)
- [x] 2.1.10 Implement employee import with merge-by-email
- [x] 2.1.11 Implement performance ratings import
- [x] 2.1.12 Implement performance reviews import
- [x] 2.1.13 Implement eNPS import

#### 2.1.C UI Components ✓ COMPLETE
- [x] 2.1.14 Create EmployeePanel component (sidebar with performance summary)
- [x] 2.1.15 Create EmployeeDetail component (full profile view)
- [x] 2.1.16 Create EmployeeEdit component (modal)
- [x] 2.1.17 Create ImportWizard component (guides through data import)

#### 2.1.D Test Data
> **Implementation Plan:** [PLAN_2.1.D_TEST_DATA.md](./PLAN_2.1.D_TEST_DATA.md)
> **Sessions Required:** 2-3 | **Est. LOC:** ~1,050 TypeScript

- [x] 2.1.18 Create test data generator script infrastructure
- [x] 2.1.19 Generate "Acme Corp" dataset (100 employees)
- [x] 2.1.20 Generate 3 review cycles with ratings + reviews
- [x] 2.1.21 Generate 3 eNPS survey responses per employee

### 2.2 Company Profile ✓ COMPLETE
- [x] 2.2.1 Create CompanySetup component
- [x] 2.2.2 Implement company table operations
- [x] 2.2.3 Require name + state during onboarding
- [x] 2.2.4 Store company data in SQLite

### 2.3 Context Builder
> **Reference:** [HR_PERSONA.md](./HR_PERSONA.md) for Claude's HR leader persona

- [x] 2.3.1 Implement context.rs with retrieval logic
- [x] 2.3.2 Add employee name/department extraction from query
- [x] 2.3.3 Build system prompt with HR persona ("Alex") + company context
- [x] 2.3.4 Include performance/eNPS data in employee context
- [x] 2.3.5 Add context to Claude API calls (+ user_name setting support)
- [x] 2.3.6 Implement context size trimming

### 2.4 Cross-Conversation Memory ✓ COMPLETE
- [x] 2.4.1 Implement memory.rs for conversation summaries
- [x] 2.4.2 Generate summaries after conversations (frontend trigger)
- [x] 2.4.3 Implement summary search/retrieval
- [x] 2.4.4 Include relevant memories in context

### 2.5 Conversation Management ✓ COMPLETE
- [x] 2.5.1 Create ConversationSidebar component
- [x] 2.5.2 Implement auto-title generation
- [x] 2.5.3 Create ConversationSearch component (FTS)
- [x] 2.5.4 Add "New conversation" action
- [x] 2.5.5 Wire sidebar to chat area

### 2.6 Stickiness Features + UI Polish
> **Implementation Plan:** `~/.claude/plans/silly-splashing-eagle.md`

#### UI Polish (from testing feedback)
- [x] 2.6.0a Add react-markdown for chat message rendering
- [x] 2.6.0b Fix email overflow in EmployeeDetail
- [x] 2.6.0c Show manager name instead of ID
- [x] 2.6.0d Make eNPS/review tiles expandable (modal)
- [x] 2.6.0e Add department and manager filters to EmployeePanel

#### Stickiness Features
- [x] 2.6.1 Create PromptSuggestions component
- [x] 2.6.2 Implement contextual prompt generation
- [x] 2.6.3 Create empty state guidance

### 2.7 Context Scaling (Query-Adaptive)
> **Architecture Doc:** [CONTEXT_SCALING_ARCHITECTURE.md](./CONTEXT_SCALING_ARCHITECTURE.md)
> **Problem:** Current 10-employee limit prevents accurate aggregate queries at scale

- [x] 2.7.0 Pass selected_employee_id from UI to context builder (prioritize selected employee)
- [x] 2.7.1 Add OrgAggregates struct and build_org_aggregates() SQL queries
- [x] 2.7.2 Implement QueryType enum and classify_query() function
- [x] 2.7.3 Refactor build_chat_context() for query-adaptive retrieval
- [x] 2.7.4 Update format functions and system prompt with aggregates
- [x] 2.7.5 Add unit tests for classification and aggregates

### Pause Point 2A ✓ VERIFIED
**Verification Required:**
- [x] Can import employee CSV/Excel and see employees with demographics
- [x] Can import performance ratings and reviews
- [x] Can import eNPS survey data
- [x] Can edit individual employee
- [x] Asking "Who's been here longest?" returns correct answer
- [x] Asking "Who's underperforming?" uses ratings data
- [x] Asking "What's our eNPS?" calculates correctly
- [x] Asking about employee by name includes their performance context
- [x] Conversation sidebar shows history
- [x] Search finds past conversations
- [x] Memory references past discussions naturally

---

## Phase 3: Protection

**Goal:** Users can't accidentally leak sensitive data

### 3.1 PII Scanner ✓ COMPLETE
- [x] 3.1.1 Implement pii.rs with regex patterns
- [x] 3.1.2 Add SSN detection (XXX-XX-XXXX, XXXXXXXXX)
- [x] 3.1.3 Add credit card detection
- [x] 3.1.4 Add bank account detection (with context)
- [x] 3.1.5 Create unit tests for PII patterns

### 3.2 Auto-Redaction ✓ COMPLETE
- [x] 3.2.1 Implement scan_and_redact function
- [x] 3.2.2 Replace PII with placeholders ([SSN_REDACTED], etc.)
- [x] 3.2.3 Return redaction list for notification

### 3.3 Notification UI ✓ COMPLETE
- [x] 3.3.1 Create PIINotification component
- [x] 3.3.2 Show brief notification on redaction
- [x] 3.3.3 Auto-dismiss after 3 seconds

### 3.4 Audit Logging ✓ COMPLETE
- [x] 3.4.1 Implement audit.rs
- [x] 3.4.2 Log redacted requests and responses
- [x] 3.4.3 Store context employee IDs used
- [x] 3.4.4 Add audit log export capability

### 3.5 Error Handling ✓ COMPLETE
- [x] 3.5.1 Create ErrorMessage component
- [x] 3.5.2 Handle API errors gracefully (categorization + user-friendly messages)
- [x] 3.5.3 Show "Retry" and "Copy Message" actions
- [x] 3.5.4 Implement read-only offline mode (ChatInput disabled when offline)

### Pause Point 3A ✓ VERIFIED
**Verification Required:**
- [x] Pasting SSN auto-redacts before sending
- [x] Notification shows briefly
- [x] Audit log captures redacted content
- [x] Offline mode allows browsing but not chatting
- [x] API errors show friendly messages

---

## Phase 4: Polish

**Goal:** Feels like a real product

### 4.1 Onboarding Flow ✓ COMPLETE
> **Implementation Plan:** `~/.claude/plans/snazzy-bubbling-boole.md`
> **Completed:** ~1,100 LOC | 11 new files | 1 session

- [x] 4.1.1 Create OnboardingContext.tsx (state + persistence)
- [x] 4.1.2 Create OnboardingFlow.tsx + StepIndicator.tsx
- [x] 4.1.3 Step 1: WelcomeStep.tsx
- [x] 4.1.4 Step 2: ApiKeyStep.tsx (wraps ApiKeyInput)
- [x] 4.1.5 Step 3: CompanyStep.tsx (wraps CompanySetup)
- [x] 4.1.6 Step 4: EmployeeImportStep.tsx (auto-loads sample data)
- [x] 4.1.7 Step 5: DisclaimerStep.tsx
- [x] 4.1.8 Step 6: TelemetryStep.tsx
- [x] 4.1.9 Step 7: FirstPromptStep.tsx
- [x] 4.1.10 App.tsx integration (replace ChatArea gating)

### 4.2 Settings Panel ✓ COMPLETE
> **Implementation Plan:** `~/.claude/plans/cozy-crafting-metcalfe.md`
> **Completed:** ~200 LOC | 6 files | 1 session

- [x] 4.2.1 Create SettingsPanel component
- [x] 4.2.2 API key management (change/remove)
- [x] 4.2.3 Company profile editing
- [x] 4.2.4 Data location display
- [x] 4.2.5 Telemetry toggle

### Pause Point 4A (Onboarding + Settings) ✓ VERIFIED
**Verification Required:**
- [x] Fresh install goes through 7-step onboarding smoothly
- [x] Onboarding resumes correctly if exited mid-flow
- [x] Sample data auto-loads on employee import step
- [x] Disclaimer checkbox required before continuing
- [x] Telemetry preference persists
- [x] Settings panel opens from main app
- [x] Can change/remove API key from settings
- [x] Can edit company profile from settings
- [x] Data location displays correctly

### 4.3 Data Export/Import ✓ COMPLETE
> **Implementation Plan:** `~/.claude/plans/delegated-sleeping-stardust.md`
> **Completed:** ~800 LOC Rust + ~280 LOC TypeScript/React | AES-256-GCM encryption | Full backup of all 9 tables

- [x] 4.3.1 Implement encrypted data export
- [x] 4.3.2 Implement data import from backup
- [x] 4.3.3 Add export/import to Settings panel

### 4.4 Monday Digest ✓ COMPLETE
> **Completed:** ~330 LOC | 8 files | 1 session

- [x] 4.4.1 Create MondayDigest component
- [x] 4.4.2 Query anniversaries from hire_date
- [x] 4.4.3 Query new hires (<90 days)
- [x] 4.4.4 Show on Monday mornings, dismissible

### Phase 4 Complete ✓

**Status:** All Phase 4 tasks complete. Pausing to review V2 features before launch.

---

## Phase V2: Intelligence & Visualization

**Goal:** Transform the app from Q&A tool into an HR intelligence platform with visual analytics, proactive signals, and structured data extraction.

> **Philosophy:** Each feature amplifies the others. Structured data extraction powers signals; signals visualize on org charts; charts persist to insight canvases.

---

### V2.1 Quick Wins (Low Complexity)

Quick polish features that improve UX without architectural changes.

#### V2.1.1 API Key Setup Guide (Enhanced) ✓ COMPLETE
> **Impact:** 🔥 High | **Completed:** 1 session

Plain-English onboarding for non-technical HR users.

- [x] V2.1.1a Add "What is an API key?" explainer with plain language
- [x] V2.1.1b Add step-by-step guide (account → billing → key → paste)
- [x] V2.1.1c Add inline key validation with error-specific guidance
- [x] V2.1.1d Add usage cost estimator ("~$5-15/month for typical use")
- [x] V2.1.1e Add troubleshooting tips for common errors

#### V2.1.2 Command Palette + Keyboard Shortcuts ✓ COMPLETE
> **Impact:** ⚡ Medium | **Completed:** 1 session

Power user polish with discoverability.

- [x] V2.1.2a Create CommandPalette component (`Cmd+K`)
- [x] V2.1.2b Add fuzzy search across actions, conversations, employees
- [x] V2.1.2c Implement core shortcuts: `Cmd+N` (new), `Cmd+/` (focus), `Cmd+E` (employees), `Cmd+,` (settings)
- [x] V2.1.2d Show keyboard hints in palette and menus

#### V2.1.3 Persona Switcher ✓ COMPLETE
> **Impact:** ⚡ Medium | **Completed:** 1 session

Pre-built HR personas for different organizational styles.

- [x] V2.1.3a Create persona definitions (Alex, Jordan, Sam, Morgan, Taylor)
- [x] V2.1.3b Add persona selector in Settings panel
- [x] V2.1.3c Create persona preview cards with tone samples
- [x] V2.1.3d Wire selected persona to system prompt

**Personas:**
| Persona | Style | Best For |
|---------|-------|----------|
| Alex (default) | Warm, practical | General HR leadership |
| Jordan | Formal, compliance-focused | Regulated industries |
| Sam | Startup-friendly, direct | Early-stage, lean HR |
| Morgan | Data-driven, analytical | Metrics-focused users |
| Taylor | Employee-advocate, empathetic | People-first cultures |

#### V2.1.4 Answer Verification Mode ✓ COMPLETE
> **Impact:** ⚡ Medium | **Completed:** 1 session

Trust but verify numeric answers.

- [x] V2.1.4a Detect numeric questions (headcount, averages, percentages)
- [x] V2.1.4b Run parallel SQL query for ground truth
- [x] V2.1.4c Display verification badge (✓ Verified / ⚠️ Check)
- [x] V2.1.4d Add "Show SQL" option for transparency

### Pause Point V2.1
**Verification Required:**
- [x] Non-technical user can complete API key setup with guide
- [x] `Cmd+K` opens palette with searchable actions
- [x] Can switch personas and see different response tones
- [x] Numeric answers show verification badges

---

### V2.2 Data Intelligence Pipeline

Foundation layer that powers visualization and signals features.

#### V2.2.1 Structured Data Extraction (Review Highlights Pipeline)
> **Impact:** 🔥 High | **Est. Sessions:** 3
> **Implementation Plan:** `~/.claude/plans/atomic-bouncing-wind.md`

Extract structured entities from performance reviews, turning prose into computable data.

- [x] V2.2.1a Design extraction schema (strengths, opportunities, quotes, sentiment)
- [x] V2.2.1b Create extraction pipeline (runs on review import)
- [x] V2.2.1c Store extracted data in new `review_highlights` table
- [x] V2.2.1d Create employee profile summaries (aggregate career narrative)
- [x] V2.2.1e Add cache invalidation on review add/edit
- [x] V2.2.1f Update context builder to use highlights instead of full reviews
- [x] V2.2.1g Auto-trigger extraction on new review import (queue pending → extract → regenerate summary)

**Extraction Schema:**
```json
{
  "strengths": ["Project leadership", "Mentoring junior devs"],
  "opportunities": ["Meeting deadlines", "Public speaking"],
  "quotes": [
    { "sentiment": "positive", "text": "Sarah was instrumental in the v2 launch..." }
  ],
  "themes": ["leadership", "technical-growth", "communication"]
}
```

**Why This Matters:** Enables sophisticated queries like "Show me all engineers who received feedback about 'meeting deadlines'" and powers Attrition Signals (#V2.4.1) with structured data instead of raw text sentiment.

#### V2.2.2 Query-Adaptive Retrieval v2
> **Impact:** ⚡ Medium | **Est. Sessions:** 2

Enhance V1's query classification with smarter context selection.

- [x] V2.2.2a Add dynamic excerpting for long content (pull relevant sentences)
- [x] V2.2.2b Implement theme-based retrieval ("common concerns" → mine themes)
- [x] V2.2.2c Add measurable token budgets by query type
- [x] V2.2.2d Add retrieval metrics (track what context was used)

**V2.2.2a Implementation Plan (Ready to Execute):**
> **Scope:** Career summary + dynamic highlight limits
> **Approach:** Inline functions in `context.rs`, `unicode-segmentation` for sentences, upfront budget calculation

| Decision | Choice |
|----------|--------|
| Excerpting targets | Career summary + recent_highlights cycle limits |
| Module location | Inline in `context.rs` (co-located with usage) |
| Sentence splitting | `unicode-segmentation` crate for accuracy |
| Budget integration | Calculate excerpt limits upfront based on employee count |

**Integration Points:**
1. `format_single_employee()` — Add `token_budget: Option<usize>` parameter
2. `format_employee_context()` — Pass per-employee budget from `TokenBudget::employee_context`
3. New helper: `excerpt_to_sentences(text, max_sentences)` using unicode segmentation
4. Dynamic `recent_highlights` limit: 3 cycles at full budget, 1-2 at reduced

### Pause Point V2.2 ✓ VERIFIED
**Verification Required:**
- [x] Imported reviews generate structured highlights (V2.2.1g auto-trigger; manual script for backfill)
- [x] Can query by extracted themes ("who has leadership feedback?")
- [x] Employee profiles show aggregated career narrative (87 summaries generated)
- [x] Long reviews don't blow token budgets (dynamic excerpting implemented)
- [x] Department detection uses word boundaries (fixed "wITh" → "IT" bug)

---

### V2.2.5 UI/UX Refinements

> **Reference:** [UI-UX-FEEDBACK.md](./UI-UX-FEEDBACK.md) — Comprehensive design review with specific file references and code recommendations.
> **Impact:** 🔥 High | **Est. Sessions:** 2-3
> **Overall Score:** 7.8/10 — Strong foundations, opportunities to elevate to excellent

Polish the visual design and accessibility before adding major new UI features.

#### V2.2.5a Critical Accessibility Fixes ✓ COMPLETE
> **Severity:** Critical | Must fix before launch

- [x] Audit and fix color contrast ratios (stone-400 → stone-500 for text)
- [x] Increase icon button touch targets to 40x40px minimum
- [x] Add visible focus styles meeting 3:1 contrast ratio

#### V2.2.5b Design Token Completion ✓ COMPLETE
> **Severity:** High | Enables consistent component styling

- [x] Complete primary color scale (add shades 200-400, 700-900)
- [x] Add custom easing curves (smooth-out, smooth-in, smooth-in-out)
- [x] Complete shadow scale (add lg, xl, 2xl)
- [x] Add letter-spacing tokens (tight, wide, wider)

#### V2.2.5c Component Consistency ✓ COMPLETE
> **Severity:** High | Foundation for V2.3 visualization components

- [x] Extract shared UI primitives (Button, Badge, Avatar, Card) to `/components/ui/`
- [x] Standardize button hover states (hover:scale-105 active:scale-95)
- [x] Standardize card hover states (unified pattern)
- [x] Decompose EmployeeDetail.tsx (619 lines → focused components)

#### V2.2.5d Motion & Reduced Motion Support ✓ COMPLETE
> **Severity:** Medium | Accessibility + polish

- [x] Add `prefers-reduced-motion` media query support
- [x] Replace button scale transforms with shadow/brightness
- [x] Slow loading spinner to 1.5s rotation

### Pause Point V2.2.5
**Verification Required:**
- [x] All text passes WCAG AA contrast (4.5:1 ratio)
- [x] Icon buttons are 40x40px minimum
- [x] Focus rings visible on all interactive elements
- [x] Shared Button/Badge/Avatar/Card components exist
- [x] Reduced motion mode disables animations

---

### V2.3 Visualization Layer

Visual analytics that transforms answers into artifacts.

#### ~~V2.3.1 Org Chart View + Heatmap Overlay~~ → DEFERRED
> **Status:** Moved to parking lot for post-launch consideration
> **Reason:** Focus on Analytics Panel for V1 launch; Org Chart adds complexity without core value

*See [KNOWN_ISSUES.md](./KNOWN_ISSUES.md) → V2 Parking Lot → Visualization for details.*

#### V2.3.2 Interactive Analytics Panel + Insight Canvas
> **Impact:** 🔥 Very High | **Est. Sessions:** 5-6

Natural language → charts with persistent insights.

**Core Analytics Panel:** ✓ COMPLETE
- [x] V2.3.2a Design analytics request schema (intent + filters + grouping)
- [x] V2.3.2b Create whitelisted NL→SQL templates (safe, deterministic)
- [x] V2.3.2c Implement chart rendering (bar, pie, line)
- [x] V2.3.2d Add "Filters applied" caption for explainability
- [x] V2.3.2e Add graceful fallback to text for non-chartable queries
- [x] V2.3.2f Wire Claude to emit structured analytics requests
- [x] V2.3.2f+ Expand chart combinations (14→24, 60% coverage) + user chart type override

**Insight Canvas (persistent workspace):**
- [x] V2.3.2g Create InsightCanvas foundation (database schema, Rust CRUD, TS types)
- [x] V2.3.2h Add "Pin to Canvas" action from analytics panel
- [x] V2.3.2i Create named boards ("Q3 Review", "Leadership Dashboard")
- [x] V2.3.2j Add chart annotation capability
- [x] V2.3.2k Add 1-page report export (combine pinned charts)
- [x] V2.3.2l Add drilldown from chart → employee list

**Technical Contract (Keep It Deterministic):**
- Claude emits **structured analytics request** (intent + filters + grouping)
- Rust runs deterministic SQLite query, returns **dataset + applied filters**
- React renders from **chart spec + dataset**
- Never let Claude generate numbers — source all aggregates from SQL

**Example Queries:**
- "Show me employee breakdown by department" → pie chart
- "Compare marketing vs sales headcount over time" → line chart
- "What's the gender breakdown on engineering?" → bar chart, pinnable

### Pause Point V2.3
**Verification Required:**
- [x] ~~Org chart renders full hierarchy~~ (DEFERRED)
- [x] ~~Heatmap overlay shows team "attention scores"~~ (DEFERRED)
- [x] Natural language query generates appropriate chart
- [x] Can pin chart to named board
- [x] Can export board as 1-page report

---

### V2.4 Intelligence Layer

Proactive insights with appropriate guardrails.

#### V2.4.1 Attrition & Sentiment Signals
> **Impact:** 🔥 High | **Est. Sessions:** 2-3
> **Depends on:** V2.2.1 (Structured Extraction)

Systemic risk identification with strong disclaimers.

- [x] V2.4.1a Define heuristic risk flags (tenure + performance + eNPS composite)
- [x] V2.4.1b Create theme mining from extracted review data
- [x] V2.4.1c Implement team-level aggregation (never individual predictions)
- [x] V2.4.1d Add "Attention Areas" summary in analytics panel
- [x] ~~V2.4.1e Wire to Org Chart heatmap overlay~~ (DEFERRED with Org Chart)
- [x] V2.4.1f Add opt-in toggle in Settings
- [x] V2.4.1g Add prominent disclaimers ("heuristic, not prediction")

**Guardrails:**
- Show patterns, not "John will leave"
- Require opt-in to enable
- Explain which factors contributed
- All outputs are team-level, anonymized

#### V2.4.2 DEI & Fairness Lens
> **Impact:** ⚡ Medium | **Est. Sessions:** 3-4
> **Depends on:** V2.2.1 (Structured Extraction)

Representation analysis with appropriate guardrails.

- [x] V2.4.2a Create representation breakdown queries (gender/ethnicity by dept/level)
- [x] V2.4.2b Add rating distribution analysis by demographic group
- [x] V2.4.2c Implement promotion delta tracking (inferred from job title keywords)
- [x] V2.4.2d Add small-n suppression (hide groups <5)
- [x] V2.4.2e Add bias disclaimers ("data may reflect historical bias")
- [x] V2.4.2f Add DEI query audit trail (query_category column)

### Pause Point V2.4 ✓ VERIFIED
**Verification Required:**
- [x] ~~Team-level attention signals appear on org chart~~ (DEFERRED with Org Chart)
- [x] Clicking team shows anonymized theme drilldown
- [x] DEI breakdown queries work with small-n suppression
- [x] All signals show appropriate disclaimers

---

### V2.4.5 Pre-Launch Audit Remediation

> **Reference:** [AUDIT-2026-02-05.md](./AUDIT-2026-02-05.md) — Full audit report (28 findings)
> **Impact:** 🔥 Critical | **Est. Sessions:** 4-6
> **Method:** Parallel multi-agent audit (security, accessibility, performance)

Address Tier 1 and Tier 2 findings from the codebase audit before launch.

#### V2.4.5a Security Hardening (Tier 1)
- [x] V2.4.5a1 Fix SQL injection in `list_employees` filter — migrate to parameterized queries (`employees.rs:317-341`)
- [x] V2.4.5a2 Enable Content Security Policy in `tauri.conf.json` (currently `null`)
- [x] V2.4.5a3 Migrate API key storage to macOS Keychain (`keyring.rs`) or update UI text
- [x] V2.4.5a4 Add `rehype-sanitize` to Markdown rendering in `MessageBubble.tsx`

#### V2.4.5b Accessibility Hardening (Tier 1)
- [x] V2.4.5b1 Add focus trap to shared `Modal.tsx` + migrate ImportWizard/EmployeeEdit modals
- [x] V2.4.5b2 Add screen reader alternatives for charts (`AnalyticsChart.tsx`) — `aria-label` + hidden data table
- [x] V2.4.5b3 Make DrilldownModal table rows keyboard-accessible (`tabIndex`, `onKeyDown`)
- [x] V2.4.5b4 Wire up form labels with `htmlFor`/`id` in EmployeeEdit FormField
- [x] V2.4.5b5 Add `aria-label`/`aria-labelledby` to Settings toggle switches

#### V2.4.5c Performance Optimization (Tier 1-2)
- [x] V2.4.5c1 Split `ConversationContext` into Data + Actions contexts to fix streaming re-renders
- [x] V2.4.5c2 Create `list_employees_with_ratings` Rust command to fix N+1 IPC calls
- [x] V2.4.5c3 Add code splitting with `React.lazy` for ImportWizard, InsightBoardView, CommandPalette
- [x] V2.4.5c4 Wrap context values in `useMemo` in both providers
- [x] V2.4.5c5 Add `React.memo` to key list components (MessageBubble, EmployeeCard, ConversationCard)

### Pause Point V2.4.5 (Audit Remediation) ✓ VERIFIED
**Verification Required:**
- [x] No SQL injection vectors (parameterized queries in all modules)
- [x] CSP enabled and tested
- [x] Modal focus trap works (Tab cycles within modal)
- [x] Charts announce data to screen readers
- [x] Streaming chat does not cause sidebar/search re-renders
- [x] Employee list loads with single IPC call (not N+1)

---

### V2.5 Import/Export Enhancements

Better data quality and workflow.

#### V2.5.1 Data Quality Center
> **Impact:** 🔥 High | **Est. Sessions:** 2-3

Pre-import validation and fix workflow.

- [x] V2.5.1a Create column mapping UI (visual drag-drop)
- [x] V2.5.1b Add header normalization preview
- [x] V2.5.1c Implement dedupe detection (email + name/DOB)
- [x] V2.5.1d Add validation rules (missing managers, invalid dates)
- [x] V2.5.1e Create "fix-and-retry" workflow (edit issues in-app)
- [x] V2.5.1f Add HRIS-specific header mappings (BambooHR, Gusto, etc.)

**Ties to Known Issue:** Fixes `file_parser::tests::test_normalize_header` by strengthening normalization rules.

### Pause Point V2.5 (Phase V2 Complete) ✓ VERIFIED
**Verification Required:**
- [x] Can map arbitrary CSV columns to fields visually
- [x] Duplicates detected and highlighted before import
- [x] Can fix validation errors in-app before committing
- [x] BambooHR export imports with auto-detected columns

---

## Phase 5: Launch

**Goal:** Real users, real feedback

### 5.1 Distribution ✓ COMPLETE
- [x] 5.1.1 Create app icon
- [x] 5.1.2 Configure macOS code signing
- [x] 5.1.3 Configure notarization
- [x] 5.1.4 Set up tauri-plugin-updater
- [x] 5.1.5 Configure GitHub Releases for updates

### Pause Point 5A (Distribution) ✓ VERIFIED
**Verification Required:**
- [x] Can export and re-import data
- [x] Monday digest appears with correct data
- [x] App is signed and notarized
- [x] Auto-update works

---

### 5.2 Trial Infrastructure (Freemium Model)
> **Reference:** [FREEMIUM-API-RESEARCH.md](./research/FREEMIUM-API-RESEARCH.md)
> **Model:** Free trial → $99 one-time purchase → BYOK required

**Trial Limits:**
| Resource | Free Trial | Paid ($99) |
|----------|------------|------------|
| AI messages | 50 (via proxy) | Unlimited (BYOK) |
| Real employees | 10 | Unlimited |
| Demo data | Included | Removable |
| Features | All unlocked | All unlocked |
| Time limit | None | None |

#### 5.2.1 API Proxy Backend ✓ COMPLETE
> **Purpose:** Fund trial AI messages without exposing your API key

- [x] 5.2.1a Choose proxy platform (Cloudflare Workers recommended)
- [x] 5.2.1b Implement device/install ID generation in app
- [x] 5.2.1c Create proxy endpoint with rate limiting
- [x] 5.2.1d Implement 50-message quota tracking per device
- [x] 5.2.1e Add your Claude API key to proxy (server-side only)

#### 5.2.2 Trial Mode in App ✓ COMPLETE
> **Purpose:** Dual-path chat routing (proxy for trial, BYOK for paid)

- [x] 5.2.2a Add `trial_mode` flag to app state
- [x] 5.2.2b Modify `chat.rs` to route via proxy OR direct BYOK
- [x] 5.2.2c Implement employee count limit (10 max in trial)
- [x] 5.2.2d Block employee add when limit reached (with upgrade prompt)
- [x] 5.2.2e Store trial message count locally

#### 5.2.3 Trial UI Components ✓ COMPLETE
> **Purpose:** Communicate limits and prompt upgrades

- [x] 5.2.3a Create TrialBanner component ("Free Trial - X messages left")
- [x] 5.2.3b Create UpgradePrompt modal (triggered at limit thresholds)
- [x] 5.2.3c Add message counter to chat header
- [x] 5.2.3d Show employee limit in EmployeePanel ("10/10 employees")
- [x] 5.2.3e Create "Upgrade" button in Settings panel

#### 5.2.4 Upgrade Flow ✓ COMPLETE
> **Purpose:** Smooth transition from trial to paid

- [x] 5.2.4a Design upgrade prompt triggers (5 messages left, 0 left, 10 employees)
- [x] 5.2.4b Link to purchase page from upgrade prompts
- [x] 5.2.4c After license entry, prompt for API key setup (use existing guide)
- [x] 5.2.4d Clear trial limits after license validation
- [x] 5.2.4e Offer demo data removal option post-purchase

### Pause Point 5B (Trial Ready) ✓ VERIFIED
**Verification Required:**
- [x] Fresh install starts in trial mode (no API key required)
- [x] Can chat using proxy (50 message limit works)
- [x] Can add up to 10 employees (limit enforced)
- [x] Message counter displays accurately
- [x] Upgrade prompts appear at thresholds (5 left, 0 left)
- [x] After purchase + API key, limits removed

---

### 5.3 License System ✓ COMPLETE
- [x] 5.3.1 Create license validation API endpoint (hrcommandcenter.com/api/validate-license)
- [x] 5.3.2 Implement license check in app (remote validation in lib.rs, fail-open)
- [x] 5.3.3 Store validation locally after success
- [x] 5.3.4 Add license input to onboarding (post-purchase flow)

### 5.4 Payment Integration ✓ COMPLETE
- [x] 5.4.1 Set up Stripe product ($99)
- [x] 5.4.2 Create checkout flow on website (Stripe Checkout via /api/checkout)
- [x] 5.4.3 Implement license key generation (crypto.randomBytes, HRC-XXXX format)
- [x] 5.4.4 Webhook copies license key to customer metadata for validation

### 5.5 Landing Page ✓ COMPLETE
- [x] 5.5.1 Update hrcommandcenter.com (deployed on Vercel)
- [x] 5.5.2 Add download links (/download page with .dmg link)
- [x] 5.5.3 Add purchase button (Hero, Header, Pricing all link to /upgrade)

### 5.5.5 Pre-Launch: Switch Stripe to Live Mode ✓ COMPLETE
> Completed 2026-02-26. Live product, price, webhook, and API keys deployed to Vercel.
- [x] 5.5.5a Toggle off "Test mode" in Stripe Dashboard
- [x] 5.5.5b Create live product + price (test products don't carry over)
- [x] 5.5.5c Copy live API keys (sk_live_..., pk_live_...)
- [x] 5.5.5d Create live webhook endpoint (checkout.session.completed → hrcommandcenter.com/api/webhook)
- [x] 5.5.5e Update Vercel env vars with all 4 live keys and redeploy

### 5.6 Beta Distribution ✓ COMPLETE
- [x] 5.6.1 Identify 5-10 beta users
- [x] 5.6.2 Distribute beta builds
- [x] 5.6.3 Set up feedback collection (in-app button)
- [x] 5.6.4 Triage and prioritize feedback

### Pause Point 5C (Launch Ready)
**Verification Required:**
- [x] Payment flow works end-to-end
- [x] License validation works
- [x] Beta users successfully using product
- [x] Critical feedback addressed

---

## Linear Checklist (All Tasks)

Copy this to external tracking if needed:

```
PHASE 0 - PRE-FLIGHT ✓ COMPLETE
[x] 0.1 Verify Rust
[x] 0.2 Verify Node
[x] 0.3 Verify Tauri CLI
[x] 0.4 Create Git repo
[x] 0.5 Document versions
[x] PAUSE 0A: Tooling verified

PHASE 1 - FOUNDATION ✓ COMPLETE
[x] 1.1.1-1.1.5 Project scaffolding (5 tasks)
[x] 1.2.1-1.2.5 SQLite setup (5 tasks)
[x] 1.3.1-1.3.6 Basic chat UI (6 tasks)
[x] 1.4.1-1.4.6 Claude API integration (6 tasks)
[x] 1.5.1-1.5.3 Network detection (3 tasks)
[x] PAUSE 1A: App runs, API works

PHASE 2 - CONTEXT ✓ COMPLETE
[x] 2.1.1-2.1.6 Employee data (6 tasks)
[x] 2.2.1-2.2.4 Company profile (4 tasks)
[x] 2.3.1-2.3.5 Context builder (5 tasks)
[x] 2.4.1-2.4.4 Cross-conversation memory (4 tasks)
[x] 2.5.1-2.5.5 Conversation management (5 tasks)
[x] 2.6.1-2.6.3 Stickiness features (3 tasks)
[x] PAUSE 2A: Context injection works

PHASE 3 - PROTECTION ✓ COMPLETE
[x] 3.1.1-3.1.5 PII scanner (5 tasks)
[x] 3.2.1-3.2.3 Auto-redaction (3 tasks)
[x] 3.3.1-3.3.3 Notification UI (3 tasks)
[x] 3.4.1-3.4.4 Audit logging (4 tasks)
[x] 3.5.1-3.5.4 Error handling (4 tasks)
[x] PAUSE 3A: PII redaction works

PHASE 4 - POLISH ✓ COMPLETE
[x] 4.1.1-4.1.10 Onboarding flow (10 tasks)
[x] 4.2.1-4.2.5 Settings panel (5 tasks)
[x] 4.3.1-4.3.3 Data export/import (3 tasks)
[x] 4.4.1-4.4.4 Monday digest (4 tasks)
[x] PAUSE 4A: Onboarding complete

PHASE V2 - INTELLIGENCE & VISUALIZATION
[x] V2.1.1a-e API Key Guide (5 tasks)
[x] V2.1.2a-d Command Palette (4 tasks)
[x] V2.1.3a-d Persona Switcher (4 tasks)
[x] V2.1.4a-d Answer Verification (4 tasks)
[x] PAUSE V2.1: Quick Wins verified
[x] V2.2.1a-g Structured Data Extraction (7 tasks)
[x] V2.2.2a-d Query-Adaptive Retrieval v2 (4 tasks)
[x] PAUSE V2.2: Data Intelligence Pipeline verified
[x] V2.2.5a Critical Accessibility Fixes (3 tasks)
[x] V2.2.5b Design Token Completion (4 tasks)
[x] V2.2.5c Component Consistency (4 tasks)
[x] V2.2.5d Motion & Reduced Motion (3 tasks)
[x] PAUSE V2.2.5: UI/UX Refinements verified
[x] V2.3.1a-j Org Chart + Heatmap (DEFERRED to parking lot)
[x] V2.3.2a-f Analytics Panel (6 tasks)
[x] V2.3.2g-l Insight Canvas (6 tasks complete)
[x] PAUSE V2.3: Visualization Layer verified
[x] V2.4.1a-g Attrition & Sentiment Signals (7 tasks, V2.4.1e deferred)
[x] V2.4.2a-f DEI & Fairness Lens (6 tasks)
[x] PAUSE V2.4: Intelligence Layer verified
[x] V2.4.5a1-a4 Security Hardening (4 tasks)
[x] V2.4.5b1-b5 Accessibility Hardening (5 tasks)
[x] V2.4.5c1-c5 Performance Optimization (5 tasks)
[x] PAUSE V2.4.5: Audit Remediation verified
[x] V2.5.1a-f Data Quality Center (6 tasks)
[x] PAUSE V2.5: Phase V2 Complete

PHASE 5 - LAUNCH
[x] 5.1.1-5.1.5 Distribution (5 tasks)
[x] PAUSE 5A: Distribution verified
[x] 5.2.1a-e API Proxy Backend (5 tasks)
[x] 5.2.2a-e Trial Mode in App (5 tasks)
[x] 5.2.3a-e Trial UI Components (5 tasks)
[x] 5.2.4a-e Upgrade Flow (5 tasks)
[x] PAUSE 5B: Trial ready
[x] 5.3.1-5.3.4 License system (4 tasks)
[x] 5.4.1-5.4.4 Payment integration (4 tasks)
[x] 5.5.1-5.5.3 Landing page (3 tasks)
[x] 5.5.5a-e Switch Stripe to live mode (5 tasks)
[x] 5.6.1-5.6.4 Beta distribution (4 tasks)
[x] PAUSE 5C: Launch ready
```

**Total: ~213 discrete tasks across 6 phases (0-4 + V2 + 5)**

---

*Last updated: February 2026*
*Session tracking: See PROGRESS.md*
