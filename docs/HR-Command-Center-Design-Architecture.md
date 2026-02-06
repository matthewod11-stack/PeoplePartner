# HR Command Center — Design & Architecture Specification

> **Purpose:** Complete design system and technical architecture for the Tauri rebuild.
> **Companion Doc:** [HR-Command-Center-Roadmap.md](./HR-Command-Center-Roadmap.md)

---

## Design Philosophy

**Core Principle:** "If it feels like software, we've failed."

Users should describe this as "talking to someone who knows HR," not "using an HR tool." Every decision—color, typography, spacing, architecture—reinforces this.

**Target Feeling:** A thoughtful mentor who explains complex HR topics over coffee—not a sterile compliance database.

---

## Key Architectural Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Database encryption | OS sandbox only | Trust macOS security, simpler stack |
| Context injection | Auto-include relevant employees | Smart retrieval, no confirmation friction |
| PII handling | Auto-redact and notify | No blocking modals, brief notification |
| Offline behavior | Read-only mode | Browse history + employees when offline |
| Memory | Cross-conversation | Compounding value over time |
| Platform | macOS only (V1) | Focus on polish, native Keychain |

---

## 1. Color System

### Palette

```
WARM NEUTRALS (90% of UI)
┌────────────────────────────────────────────────────────┐
│  stone-50   #FAFAF9   Background (warm off-white)      │
│  stone-100  #F5F5F4   Surface (cards, panels)          │
│  stone-200  #E7E5E4   Borders                          │
│  stone-400  #A8A29E   Muted text (timestamps)          │
│  stone-500  #78716C   Secondary text                   │
│  stone-700  #44403C   Primary text                     │
│  stone-900  #1C1917   Headings, emphasis               │
└────────────────────────────────────────────────────────┘

PRIMARY ACCENT (10% of UI)
┌────────────────────────────────────────────────────────┐
│  teal-500   #0D9488   Primary actions, links           │
│  teal-600   #0F766E   Hover states                     │
│  teal-100   #CCFBF1   Highlights, selected states      │
└────────────────────────────────────────────────────────┘

SEMANTIC
┌────────────────────────────────────────────────────────┐
│  success    #22C55E   Confirmations                    │
│  warning    #F59E0B   Caution, PII redacted            │
│  error      #EF4444   Errors, destructive actions      │
└────────────────────────────────────────────────────────┘
```

### Color Rules

1. **90% neutrals, 10% color** — Creates calm focus
2. **Color signals meaning, not decoration** — Teal = actions, success = confirmations
3. **Never use color alone** — Pair with icons/text for accessibility
4. **Avoid corporate blue (#0066CC)** — Feels like IT software

---

## 2. Typography

### Font Stack

```css
/* System fonts for performance and familiarity */
font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', system-ui, sans-serif;

/* Monospace (for any code/data display) */
font-family: 'SF Mono', 'Menlo', 'Monaco', monospace;
```

### Type Scale

| Name | Size | Line Height | Use |
|------|------|-------------|-----|
| `text-xs` | 12px | 16px | Timestamps, metadata |
| `text-sm` | 14px | 20px | Labels, secondary text |
| `text-base` | 16px | 24px | Body text, chat messages (DEFAULT) |
| `text-lg` | 18px | 28px | Subheadings, emphasis |
| `text-xl` | 20px | 28px | Section titles |

### Typography Rules

1. **16px minimum base** — Non-technical users need readability
2. **Weights: 400 (body), 500 (emphasis), 600 (headings)** — Skip 700+
3. **Line height 1.5-1.6 for body** — Comfortable reading
4. **60-70 characters max width** — Prevents eye strain

---

## 3. Spacing System

### Scale (4px base)

| Token | Value | Use |
|-------|-------|-----|
| `space-1` | 4px | Tight internal spacing |
| `space-2` | 8px | Icon gaps, compact padding |
| `space-3` | 12px | Input padding, small gaps |
| `space-4` | 16px | Standard padding, element gaps |
| `space-6` | 24px | Card padding, larger gaps |
| `space-8` | 32px | Section separators |
| `space-12` | 48px | Page margins |

### Common Patterns

```
Button padding:       space-3 horizontal, space-2 vertical
Card padding:         space-6
Chat message gap:     space-4 (same speaker), space-6 (different speaker)
Input field padding:  space-3
Window margin:        space-6
Sidebar width:        240px (conversations list)
```

---

## 4. Component Architecture

### Component List (19 total)

```
LAYOUT (4)
├── AppShell          Main window container
├── ChatLayout        Chat area with input
├── ConversationSidebar  Left panel with conversation list
└── SettingsPanel     Settings overlay

CHAT (5)
├── ChatInput         Text input + send button
├── MessageBubble     Single message (user/assistant)
├── MessageList       Scrollable message container
├── TypingIndicator   "Thinking..." animation
└── PromptSuggestions Contextual suggestions when input empty

PRIMITIVES (5)
├── Button            Primary/secondary/ghost variants
├── Input             Text input with label
├── Card              Content container
├── Badge             Status indicators
└── IconButton        Icon-only buttons

FEATURES (5)
├── EmployeeContext   Side panel with employee data
├── FileDropzone      CSV import area
├── ApiKeyInput       Masked API key field + validation
├── MondayDigest      Proactive weekly suggestions
└── PIINotification   Brief auto-redact notification
```

### Message Bubble Specs

```
USER MESSAGE
┌─────────────────────────────────────────┐
│  How do I handle an employee who is    │  Right-aligned
│  consistently late to work?            │  Teal background (#0D9488)
│                              2:34 PM   │  White text
└─────────────────────────────────────────┘  radius: 16px

ASSISTANT MESSAGE
┌─────────────────────────────────────────┐
│  Here's a step-by-step approach:       │  Left-aligned
│  1. Document the pattern first...      │  Stone-100 background
│                              2:35 PM   │  Stone-900 text
└─────────────────────────────────────────┘  radius: 16px

Max width: 80% of container
Padding: space-4 (16px)
```

---

## 5. Layout Structure

### Main Window (1200 x 800 default)

```
┌──────────────────────────────────────────────────────────────────────┐
│ [icon] HR Command Center                           [?] [gear]         │
├────────────────┬─────────────────────────────┬───────────────────────┤
│                │                             │                       │
│  CONVERSATIONS │       CHAT AREA             │   EMPLOYEE CONTEXT    │
│  (collapsible) │       (primary)             │   (collapsible 25%)   │
│                │                             │                       │
│  [+ New]       │  ┌───────────────────────┐ │   Sarah Chen          │
│                │  │ User message...       │ │   Marketing Manager   │
│  Today         │  └───────────────────────┘ │   California          │
│  • Performance │                             │   Hired: 2021         │
│  • Sarah's PTO │  ┌───────────────────────┐ │                       │
│                │  │ Assistant response... │ │                       │
│  Yesterday     │  └───────────────────────┘ │                       │
│  • Hiring help │                             │                       │
│                │                             │                       │
├────────────────┼─────────────────────────────┼───────────────────────┤
│  [Search...]   │ ┌───────────────────────┐  │   [Import CSV]        │
│                │ │ Ask a question...  [→]│  │                       │
│                │ └───────────────────────┘  │                       │
│                │ [Who's been here longest?] │                       │
│                │ [Help with performance..] │                       │
└────────────────┴─────────────────────────────┴───────────────────────┘
```

### Window Constraints

| Constraint | Value |
|------------|-------|
| Minimum | 800 x 600px |
| Default | 1200 x 800px |
| Chat max-width | 720px (centered when full-width) |
| Context panel | 280px min, collapsible |
| Conversations sidebar | 240px, collapsible |

---

## 6. Micro-Interactions

### Animation Timing

| Type | Duration | Easing |
|------|----------|--------|
| Instant feedback | 100ms | ease-out |
| Hover states | 200ms | ease-out |
| Panel reveals | 300ms | ease-in-out |
| Notifications | 3000ms | (auto-dismiss) |

### Key Interactions

**Message Send:**
1. Input fades to 50% (100ms)
2. Bubble scales 0.95 → 1.0 (200ms)
3. Scroll to new message (200ms)

**Typing Indicator:**
- Three dots, staggered fade animation
- Loop duration: 1200ms

**Button Hover:**
- Scale 1.0 → 1.02
- Shadow lift
- Background darkens 8%

**PII Notification:**
- Slide in from top (200ms)
- Warning color background
- Auto-dismiss after 3 seconds
- "SSN redacted for your protection"

### Error States

```
┌─────────────────────────────────────────┐
│  ⚠️ Can't reach Claude                  │
│                                         │
│  Your message is saved. Check your      │
│  internet connection and try again.     │
│                                         │
│  [Retry Now]  [Copy Message]            │
└─────────────────────────────────────────┘

┌─────────────────────────────────────────┐
│  📴 You're offline                       │
│                                         │
│  You can still browse your employees    │
│  and past conversations.                │
│                                         │
│  Chat will resume when connected.       │
└─────────────────────────────────────────┘
```

---

## 7. Technical Architecture

### Stack

| Layer | Technology | Why |
|-------|------------|-----|
| Framework | Tauri 2.0 | 5MB bundle, native SQLite, secure |
| Frontend | React + Vite | Fast, familiar, simple |
| Database | SQLite (SQLx) | Local, no cloud dependency |
| AI | Anthropic Claude | Best for nuanced HR conversations |
| Styling | Tailwind CSS | Utility-first, small bundle |
| Platform | macOS only | Focus, native Keychain |

### Communication Pattern

```
┌─────────────────────┐    invoke()    ┌─────────────────────┐
│  React Frontend     │ ◄────────────► │  Rust Backend       │
│                     │                │                     │
│  • Chat UI          │                │  • SQLite (SQLx)    │
│  • Employee Table   │                │  • PII Scanner      │
│  • Conversation List│                │  • Context Builder  │
│  • Settings         │                │  • API Key Store    │
│  • CSV Import       │                │  • Claude API       │
│                     │                │  • Audit Logger     │
└─────────────────────┘                └─────────────────────┘
```

**Key principle:** Frontend calls business operations via `invoke()`, NOT raw SQL. All sensitive operations happen in Rust.

### Data Flow: Chat with Context

```
User Input
    ↓
Network Check → If offline, show read-only message
    ↓
PII Scan (Rust) → Auto-redact, notify frontend
    ↓
Context Builder (Rust) → Query relevant employees, add company/state context
    ↓
Memory Lookup (Rust) → Find relevant past conversation summaries
    ↓
Claude API (Rust) → API key from Keychain, stream response
    ↓
Audit Log (Rust) → Store redacted request/response
    ↓
Response → Display in React, store conversation
    ↓
Generate Summary → For cross-conversation memory
```

---

## 8. Context Builder

### Purpose
Automatically determine which employee data and context to include with each query, without requiring user confirmation.

### Retrieval Logic

```rust
pub fn build_context(query: &str, company: &Company) -> ContextPayload {
    let mut context = ContextPayload::new();

    // 1. Always include company context
    context.add_company(company.name, company.state);

    // 2. Find relevant employees by name/department mention
    let mentioned = extract_employee_mentions(query);
    for employee in mentioned {
        context.add_employee(employee);
    }

    // 3. If no specific mentions, include recent/relevant employees
    if mentioned.is_empty() && query_needs_employee_context(query) {
        let relevant = get_relevant_employees(query, limit: 10);
        for employee in relevant {
            context.add_employee(employee);
        }
    }

    // 4. Add relevant past conversation summaries
    let memory = find_relevant_memories(query, limit: 3);
    for summary in memory {
        context.add_memory(summary);
    }

    // 5. Check context size, trim if needed
    context.trim_to_token_limit(MAX_CONTEXT_TOKENS);

    context
}
```

### System Prompt Template

```
You are an HR assistant for {company_name}, a company based in {state}.

When answering HR questions, consider {state} employment law. Flag when federal vs state law differs.

IMPORTANT: This is informational guidance only, not legal advice. The user has acknowledged this.

COMPANY CONTEXT:
- Company: {company_name}
- State: {state}
- Industry: {industry} (if provided)

EMPLOYEE DATA:
{employee_context}

PAST CONVERSATIONS (for reference):
{memory_summaries}

Answer questions naturally, referencing specific employees when relevant.
```

---

## 9. Cross-Conversation Memory

### Purpose
Allow Claude to reference past conversations naturally, creating compounding value over time.

### Implementation

```rust
// After each conversation ends or significant exchange
pub fn generate_summary(conversation: &Conversation) -> String {
    // Use Claude to generate 2-3 sentence summary
    // Focus on: topics discussed, employees mentioned, decisions/actions
    let prompt = format!(
        "Summarize this HR conversation in 2-3 sentences. \
         Include: main topic, any employees mentioned, key outcomes.\n\n{}",
        conversation.messages_text()
    );

    call_claude_for_summary(prompt)
}

// When building context for new query
pub fn find_relevant_memories(query: &str, limit: usize) -> Vec<ConversationSummary> {
    // Search summaries for keyword matches
    // Prioritize: same employees mentioned, similar topics, recency
    db.search_conversation_summaries(query, limit)
}
```

### Example Behavior

**User:** "Sarah is late again"

**Context includes:** Summary from March conversation about Sarah's tardiness

**Claude responds:** "I remember we discussed Sarah's tardiness pattern in March. You documented two verbal warnings then. Based on your progressive discipline approach, a written warning would be the next step. Would you like me to draft one?"

---

## 10. SQLite Schema

```sql
-- 5 tables

-- Employees: The core data
CREATE TABLE employees (
    id TEXT PRIMARY KEY,
    email TEXT UNIQUE NOT NULL,
    full_name TEXT NOT NULL,
    department TEXT,
    job_title TEXT,
    manager_id TEXT,
    hire_date TEXT,
    work_state TEXT,           -- Primary work location state
    status TEXT DEFAULT 'active',  -- active, terminated, leave
    extra_fields TEXT,         -- JSON for flexible fields
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP
);

-- Conversations: Chat history with metadata
CREATE TABLE conversations (
    id TEXT PRIMARY KEY,
    title TEXT,                -- Auto-generated from first message
    summary TEXT,              -- For cross-conversation memory
    messages_json TEXT NOT NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP
);

-- Company: Required profile
CREATE TABLE company (
    id TEXT PRIMARY KEY DEFAULT 'default',
    name TEXT NOT NULL,
    state TEXT NOT NULL,
    industry TEXT,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP
);

-- Settings: App config (non-secret)
CREATE TABLE settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP
);

-- Audit Log: What was sent to AI
CREATE TABLE audit_log (
    id TEXT PRIMARY KEY,
    conversation_id TEXT,
    request_redacted TEXT NOT NULL,   -- User message with PII replaced
    response_text TEXT NOT NULL,
    context_used TEXT,                -- JSON of employee IDs used
    created_at TEXT DEFAULT CURRENT_TIMESTAMP
);

-- Indexes
CREATE INDEX idx_employees_department ON employees(department);
CREATE INDEX idx_employees_status ON employees(status);
CREATE INDEX idx_employees_work_state ON employees(work_state);
CREATE INDEX idx_conversations_updated ON conversations(updated_at);
CREATE INDEX idx_audit_log_created ON audit_log(created_at);

-- Full-text search for conversations
CREATE VIRTUAL TABLE conversations_fts USING fts5(
    title,
    messages_json,
    summary,
    content='conversations',
    content_rowid='rowid'
);
```

### What's NOT in SQLite

| Data | Location | Why |
|------|----------|-----|
| API keys | macOS Keychain | OS-level encryption |
| License validation | Local flag + server | One-time check |

---

## 11. Project Structure

```
hr-command-center/
├── src/                          # React (~2,200 LOC)
│   ├── App.tsx
│   ├── main.tsx
│   ├── contexts/
│   │   ├── ChatContext.tsx
│   │   ├── EmployeeContext.tsx
│   │   ├── ConversationContext.tsx
│   │   └── AppContext.tsx
│   ├── components/
│   │   ├── chat/
│   │   │   ├── ChatInterface.tsx
│   │   │   ├── MessageList.tsx
│   │   │   ├── ChatInput.tsx
│   │   │   └── PromptSuggestions.tsx
│   │   ├── conversations/
│   │   │   ├── ConversationSidebar.tsx
│   │   │   └── ConversationSearch.tsx
│   │   ├── employees/
│   │   │   ├── EmployeePanel.tsx
│   │   │   ├── EmployeeEdit.tsx
│   │   │   └── CSVImport.tsx
│   │   ├── onboarding/
│   │   │   ├── OnboardingFlow.tsx
│   │   │   └── CompanySetup.tsx
│   │   ├── settings/
│   │   │   └── SettingsPanel.tsx
│   │   └── shared/
│   │       ├── PIINotification.tsx
│   │       ├── MondayDigest.tsx
│   │       └── ErrorState.tsx
│   ├── hooks/
│   │   ├── useChat.ts
│   │   ├── useEmployees.ts
│   │   ├── useConversations.ts
│   │   └── useNetwork.ts
│   ├── lib/
│   │   ├── types.ts
│   │   └── tauri-commands.ts
│   └── styles/
│       └── globals.css
│
├── src-tauri/                    # Rust (~1,200 LOC)
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── src/
│   │   ├── main.rs
│   │   ├── lib.rs
│   │   ├── db.rs                 # SQLite queries
│   │   ├── chat.rs               # API calls, streaming
│   │   ├── context.rs            # Context builder
│   │   ├── memory.rs             # Conversation summaries
│   │   ├── employees.rs          # Employee CRUD
│   │   ├── pii.rs                # PII detection + redaction
│   │   ├── audit.rs              # Audit logging
│   │   └── keyring.rs            # API key + license storage
│   └── migrations/
│       └── 001_initial.sql
│
├── package.json
├── vite.config.ts
├── tailwind.config.js
└── tsconfig.json
```

### LOC Budget

| Area | Target LOC |
|------|------------|
| React components | ~1,400 |
| React contexts/hooks | ~600 |
| React utilities | ~100 |
| Rust backend | ~1,200 |
| **Total** | **~3,300** |

---

## 12. Security Architecture

### Threat Model (V1)

**In scope (protected against):**
- PII accidentally sent to AI (auto-redacted)
- API keys exposed to frontend (Keychain only)
- Raw SQL injection (all queries in Rust)
- Unauthorized file access (Tauri capabilities)

**Out of scope (accepted risks):**
- Local user with device access can read SQLite
- No encryption at rest (OS sandbox provides protection)
- License key sharing (honor system for target market)

### API Key Storage

```rust
// Uses macOS Keychain via tauri-plugin-keyring
// Keys NEVER exposed to frontend

pub fn store_api_key(provider: &str, key: &str) -> Result<(), Error> {
    let entry = Entry::new("hr-command-center", provider)?;
    entry.set_password(key)
}

// Only called from Rust when making API requests
pub fn get_api_key(provider: &str) -> Result<String, Error> {
    let entry = Entry::new("hr-command-center", provider)?;
    entry.get_password()
}

// Validate on entry
pub async fn validate_api_key(key: &str) -> Result<bool, Error> {
    // Make minimal test call to Claude
    let response = claude_test_call(key).await;
    Ok(response.is_ok())
}
```

### PII Scanning & Auto-Redaction

```rust
// Runs BEFORE any data sent to Claude
// Auto-redacts and returns notification for frontend

pub fn scan_and_redact(text: &str) -> (String, Vec<PIIRedaction>) {
    let mut redacted = text.to_string();
    let mut redactions = Vec::new();

    // SSN patterns: XXX-XX-XXXX or XXXXXXXXX
    for cap in SSN_PATTERN.captures_iter(text) {
        redacted = redacted.replace(&cap[0], "[SSN_REDACTED]");
        redactions.push(PIIRedaction::SSN);
    }

    // Credit card patterns
    for cap in CC_PATTERN.captures_iter(&redacted) {
        redacted = redacted.replace(&cap[0], "[CC_REDACTED]");
        redactions.push(PIIRedaction::CreditCard);
    }

    // Bank account (with context)
    if BANK_CONTEXT.is_match(&redacted) {
        for cap in BANK_NUMBER.captures_iter(&redacted) {
            redacted = redacted.replace(&cap[0], "[BANK_REDACTED]");
            redactions.push(PIIRedaction::BankAccount);
        }
    }

    (redacted, redactions)
}
```

### Audit Logging

```rust
pub fn log_interaction(
    conversation_id: &str,
    request: &str,       // Already redacted
    response: &str,
    context_employee_ids: Vec<String>,
) -> Result<(), Error> {
    db.insert_audit_log(AuditEntry {
        id: generate_id(),
        conversation_id: conversation_id.to_string(),
        request_redacted: request.to_string(),
        response_text: response.to_string(),
        context_used: serde_json::to_string(&context_employee_ids)?,
        created_at: Utc::now(),
    })
}
```

### Security Checklist

- [x] API keys in Keychain, not SQLite
- [x] PII auto-redacted in Rust before API calls
- [x] No raw SQL exposed to frontend
- [x] Tauri capabilities restrict file access
- [x] Only HTTPS allowed for external requests
- [x] Audit log stores redacted content only
- [x] License validated once, works offline after

---

## 13. Offline Mode

### Behavior

When network is unavailable:

| Feature | Behavior |
|---------|----------|
| Chat input | Disabled with message |
| Past conversations | Browsable |
| Employee data | Viewable and editable |
| Conversation search | Works (local) |
| CSV import | Works (local) |
| Settings | Accessible |

### Implementation

```rust
pub fn check_network() -> bool {
    // Quick connectivity check
    std::net::TcpStream::connect("api.anthropic.com:443")
        .map(|_| true)
        .unwrap_or(false)
}
```

```tsx
// Frontend hook
function useNetwork() {
    const [isOnline, setIsOnline] = useState(true);

    useEffect(() => {
        const check = async () => {
            const online = await invoke('check_network');
            setIsOnline(online);
        };

        check();
        const interval = setInterval(check, 30000); // Check every 30s

        return () => clearInterval(interval);
    }, []);

    return isOnline;
}
```

---

## 14. License Validation

### Flow

```
1. User purchases → Receives license key via email
2. First launch → Onboarding asks for license key
3. App sends key to validation endpoint (one time)
4. Server returns: { valid: true, email: "...", expires: null }
5. App stores validation locally
6. Future launches → Check local flag, never call server again
```

### Implementation

```rust
pub async fn validate_license(key: &str) -> Result<LicenseStatus, Error> {
    // Check if already validated
    if let Some(status) = get_stored_license_status() {
        return Ok(status);
    }

    // One-time server validation
    let response = reqwest::Client::new()
        .post("https://api.hrcommandcenter.com/validate")
        .json(&json!({ "key": key }))
        .send()
        .await?;

    let status: LicenseStatus = response.json().await?;

    if status.valid {
        store_license_status(&status)?;
    }

    Ok(status)
}
```

---

## 15. Telemetry (Opt-in)

### Philosophy
Privacy-first: telemetry is opt-in during onboarding, anonymized, and focused on crashes/errors only.

### What's Collected (if opted in)
- Crash reports (stack traces, no user data)
- Error types (API failures, not content)
- App version
- macOS version

### What's NEVER Collected
- Chat content
- Employee data
- Company information
- API keys
- User identity

### Implementation
```rust
// Only if user opted in during onboarding
pub fn report_error(error: &Error) {
    if !is_telemetry_enabled() {
        return;
    }

    sentry::capture_error(error);
}
```

---

## 16. What NOT to Build

| Pattern | Why Avoid | Alternative |
|---------|-----------|-------------|
| Zustand/Redux | Overkill for 6 views | React Context |
| GraphQL | Massive overhead | Tauri commands |
| ORM (Diesel) | Complex macros | Raw SQLx |
| React Router | 6 views max | Conditional rendering |
| Dark mode (v1) | Ship light first | Add if requested |
| Multiple themes | One good theme | Keep it simple |
| Navigation sidebar | Minimal UI | Conversation list only |
| Toast system | Inline feedback | Simple notifications |

---

## 17. Accessibility Checklist

### Visual
- [x] 16px minimum font size
- [x] 4.5:1 contrast ratio (WCAG AA)
- [x] No information by color alone
- [x] Visible focus indicators (2px teal ring)

### Keyboard
- [x] All elements focusable
- [x] Logical tab order
- [x] Enter/Space activates buttons
- [x] Escape closes modals
- [x] Chat input auto-focused on launch

### Screen Readers
- [x] Semantic HTML (button, input, main)
- [x] ARIA labels for icon buttons
- [x] Live regions for new messages
- [x] Announcements for PII redaction notifications

---

## 18. Design Tokens (Copy-Paste Ready)

### Tailwind Config Extension

```javascript
// tailwind.config.js
module.exports = {
  theme: {
    extend: {
      colors: {
        primary: {
          50: '#F0FDFA',
          100: '#CCFBF1',
          500: '#0D9488',
          600: '#0F766E',
        },
        stone: {
          50: '#FAFAF9',
          100: '#F5F5F4',
          200: '#E7E5E4',
          400: '#A8A29E',
          500: '#78716C',
          700: '#44403C',
          900: '#1C1917',
        },
      },
      fontFamily: {
        sans: ['-apple-system', 'BlinkMacSystemFont', 'Segoe UI', 'system-ui', 'sans-serif'],
        mono: ['SF Mono', 'Menlo', 'Monaco', 'monospace'],
      },
      fontSize: {
        xs: ['12px', { lineHeight: '16px' }],
        sm: ['14px', { lineHeight: '20px' }],
        base: ['16px', { lineHeight: '24px' }],
        lg: ['18px', { lineHeight: '28px' }],
        xl: ['20px', { lineHeight: '28px' }],
      },
      borderRadius: {
        sm: '4px',
        md: '8px',
        lg: '12px',
        xl: '16px',
      },
      boxShadow: {
        sm: '0 1px 2px 0 rgb(0 0 0 / 0.05)',
        md: '0 4px 6px -1px rgb(0 0 0 / 0.1)',
      },
      width: {
        'sidebar': '240px',
        'context-panel': '280px',
      },
    },
  },
}
```

---

## 19. Inspiration References

### Study These
- **Linear** — Refined minimalism, keyboard-first
- **Notion** — Warm grays, friendly empty states
- **Raycast** — Speed, zero-friction interaction
- **Hey** — Warm accent colors, human microcopy
- **Arc** — Generous rounding, spring animations

### Avoid These Vibes
- **Slack** — Too noisy, overwhelming sidebar
- **Microsoft Teams** — Corporate blue, cramped
- **Zendesk** — Support chat aesthetic (transactional)

---

## Final Mantra

> "Your company's HR brain—private, always in context, always ready to help."

Every design and architecture decision should make the app feel more like a trusted colleague and less like software.

**Trust over friction.** Auto-include, auto-redact, auto-remember.

---

*Generated: December 2025*
*Stack: Tauri + React + Vite + SQLite*
*Target: ~3,300 lines of code*
*Platform: macOS only (V1)*
