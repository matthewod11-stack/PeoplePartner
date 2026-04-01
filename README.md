# People Partner

> Your company's HR brain -- private, always in context, always ready to help.

People Partner is a desktop AI assistant for HR professionals. It keeps your employee data on your Mac while providing intelligent, context-aware guidance on policies, compliance, and people decisions.

**Website:** [peoplepartner.io](https://peoplepartner.io)

---

## What It Does

- **Knows Your Company** -- Import employee data and company documents. Get answers that understand your specific context, not generic advice.
- **Remembers Conversations** -- References past discussions naturally, building institutional knowledge over time.
- **Protects Sensitive Data** -- PII auto-redaction, audit trails, and local-first storage. Nothing leaves your Mac without your knowledge.
- **Works Offline** -- Browse employees, review past conversations, and access your data even without internet.

## Who It's For

Solo people-ops professionals, founders handling HR themselves, and small HR teams at companies with 10-200 employees. If you've ever Googled an employment law question at 11pm, this is for you.

## Tech Stack

| Layer | Technology |
|-------|------------|
| Framework | [Tauri 2](https://tauri.app/) |
| Frontend | React, TypeScript, Tailwind CSS |
| Backend | Rust, SQLite |
| AI | Multi-provider (Claude, OpenAI, Gemini) -- BYOK |
| Platform | macOS (Apple Silicon + Intel) |
| Security | macOS Keychain, PII redaction, encrypted backups |

## Privacy

- All data stored locally in SQLite on your Mac
- API keys stored in macOS Keychain
- PII auto-redacted before sending to any AI provider
- Full audit log of all AI interactions
- No telemetry, no cloud sync, no third-party data sharing

## Getting Started

1. Purchase a license at [peoplepartner.io](https://peoplepartner.io) ($99, one-time)
2. Download the .dmg for your Mac (Apple Silicon or Intel)
3. Install, activate with your license key, and add your API key
4. Import your employee data and start asking questions

## License

Proprietary. See [LICENSE](LICENSE) for details.
