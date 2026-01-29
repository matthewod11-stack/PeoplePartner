# Freemium Model Research: API Key Handling for Desktop AI Apps

> **Context:** HR Command Center needs a try-before-you-buy model where users can test with demo data and limited employees before paying. The challenge is handling Claude API access during the free trial.

---

## Summary of Approaches

| Model | How It Works | Examples | Pros | Cons |
|-------|--------------|----------|------|------|
| **BYOK Only** | Users bring their own API key from day one | TypingMind, BoltAI | Zero API costs for you, privacy-first | High friction, users won't test without key |
| **Credits Included** | You pay for API on user's behalf, bundle into price | Cursor Pro, Raycast Pro | Seamless UX, no friction | You absorb API costs, need backend |
| **Hybrid BYOK + Credits** | Provide trial credits, then BYOK or subscription | Warp, Raycast | Best of both worlds | More complex to implement |
| **User-Pays** | Users create account with AI provider, you route through | Puter.js | Zero cost to you | Extremely high friction |
| **Tiered Free/Paid** | Free tier with limits, paid removes limits | Cursor Hobby/Pro | Try before buy | Still need API solution for free tier |

---

## Detailed Examples

### 1. Cursor IDE (Most Relevant)

**Free Tier (Hobby):**
- ~2,000 code completions
- 50 "slow" premium model requests
- 2-week Pro trial included

**Pro ($20/month):**
- Unlimited completions
- $20/month credit pool for premium models
- Overages billed at API cost

**Key Insight:** Cursor provides real AI functionality in free tier but rate-limits it. Users can test meaningfully before paying.

**BYOK Option:** Available but disables some features (agent mode). Cursor prioritizes their integrated experience.

---

### 2. Raycast AI

**Free:**
- 50 free AI messages to try any Pro model
- OR use your own API key (BYOK) unlimited

**Pro ($8/month):**
- Unlimited AI with their models
- Advanced models as add-on

**Key Insight:** 50 messages is enough to hook users. BYOK available for power users who want to avoid subscription.

---

### 3. Warp Terminal

**Hybrid Approach:**
- Offers BYOK for power users
- Provides credits for those without keys
- Automatic fallback: If BYOK fails, uses Warp credits

**Key Insight:** Fallback mechanism prevents frustration. Users don't need to understand API economics.

---

### 4. BoltAI (macOS)

**Pricing Options:**
- $29 one-time + BYOK (you provide API key)
- $5/month subscription (AI credits included)
- $69 one-time + BYOK (more features)

**Key Insight:** Offers both models side-by-side. Some users prefer one-time purchase + BYOK, others prefer simplicity of subscription.

---

### 5. TypingMind

**Model:** One-time purchase + BYOK only
- $79 lifetime license
- Users provide their own API keys
- Works with OpenAI, Claude, Google, etc.

**Key Insight:** Their pitch is "pay only for what you use" — appeals to cost-conscious power users. But requires API key from the start.

---

## The API Proxy Architecture

For providing credits without exposing your API key:

### How AIProxy Works:
1. Your master API key never touches the client
2. Key is split-encrypted: half in server, half in app
3. Requests from app marry both halves to derive key
4. Calls made server-side, results returned to app

### Simple Proxy Pattern:
```
[Desktop App] → [Your Backend/Lambda] → [Claude API]
                      ↓
              [Rate Limit + Auth Check]
```

### Implementation Options:
- **AWS Lambda + API Gateway** - Pay per request, scales automatically
- **Cloudflare Workers** - Edge deployment, very low latency
- **Simple VPS** - More control, fixed cost

---

## Recommended Approach for HR Command Center

Given your constraints (macOS app, $99 one-time purchase, privacy-focused):

### **Option A: Tiered Hybrid (Recommended)**

```
┌─────────────────────────────────────────────────────────┐
│  FREE TIER (No Purchase Required)                       │
├─────────────────────────────────────────────────────────┤
│  • Demo data pre-loaded (fake company, 25 employees)    │
│  • Add up to 3 real employees                           │
│  • 50 AI messages included (via your proxy)             │
│  • All features available to test                       │
│  • Watermark or banner: "Free Trial - X messages left"  │
└─────────────────────────────────────────────────────────┘
                          ↓ Purchase $99
┌─────────────────────────────────────────────────────────┐
│  PAID TIER                                              │
├─────────────────────────────────────────────────────────┤
│  • Unlimited employees                                  │
│  • Demo data removable                                  │
│  • BYOK required (user's Claude API key)                │
│  • Or: Monthly AI credits add-on ($5/month optional)    │
└─────────────────────────────────────────────────────────┘
```

### Why This Works:
1. **Zero friction to try** — Download and go, demo data ready
2. **Real AI experience** — 50 messages lets them actually use Alex
3. **Clear value prop** — They see it works before paying
4. **Sustainable** — After purchase, they pay their own API costs
5. **Privacy preserved** — BYOK means you never see their data

### Cost Estimate (Your Side):
- 50 messages ≈ $0.15-0.50 per user (Claude Sonnet)
- 1000 trial users/month = $150-500/month
- Conversion rate 5-10% = 50-100 sales = $4,950-9,900
- **Healthy margin even with free trial**

---

### **Option B: BYOK Only with Generous Demo**

```
┌─────────────────────────────────────────────────────────┐
│  FREE TIER (No API Key Required)                        │
├─────────────────────────────────────────────────────────┤
│  • Demo data pre-loaded                                 │
│  • Add up to 3 real employees                           │
│  • AI features DISABLED (greyed out)                    │
│  • Preview what Alex can do (canned examples)           │
│  • "Add API key to unlock" prompts                      │
└─────────────────────────────────────────────────────────┘
                          ↓ Add API Key
┌─────────────────────────────────────────────────────────┐
│  FULL APP (With API Key)                                │
├─────────────────────────────────────────────────────────┤
│  • All features unlocked                                │
│  • Purchase $99 to remove employee limit                │
└─────────────────────────────────────────────────────────┘
```

### Why This Might Work:
- Zero ongoing costs for you
- Appeals to privacy-focused HR audience
- But: Higher friction, lower conversion

---

### **Option C: Time-Limited Full Trial**

```
┌─────────────────────────────────────────────────────────┐
│  14-DAY TRIAL (Your API Key)                            │
├─────────────────────────────────────────────────────────┤
│  • Full functionality                                   │
│  • 200 AI messages included                             │
│  • Demo data + up to 10 employees                       │
│  • Clock visible: "12 days remaining"                   │
└─────────────────────────────────────────────────────────┘
                          ↓ Trial ends or purchase
┌─────────────────────────────────────────────────────────┐
│  PURCHASE REQUIRED                                      │
├─────────────────────────────────────────────────────────┤
│  • $99 one-time + BYOK                                  │
│  • Read-only mode if trial expires without purchase     │
└─────────────────────────────────────────────────────────┘
```

---

## Implementation Requirements

### For Option A (Recommended):

1. **Simple API Proxy Backend**
   - Cloudflare Worker or AWS Lambda
   - Rate limiting per install (device ID)
   - 50 message quota tracking
   - Your Claude API key stored server-side

2. **License Verification**
   - Generate license key on purchase
   - Verify on app launch (phone home)
   - Grace period for offline use

3. **App Changes**
   - Add trial message counter
   - Add "Upgrade" prompts
   - Support both proxy (trial) and BYOK (paid) modes
   - Demo data seeder

### Minimal Backend (Cloudflare Worker Example):
```javascript
// Pseudocode
export default {
  async fetch(request) {
    const deviceId = request.headers.get('X-Device-ID');
    const messageCount = await getMessageCount(deviceId);

    if (messageCount >= 50) {
      return new Response('Trial limit reached', { status: 402 });
    }

    const response = await fetch('https://api.anthropic.com/v1/messages', {
      headers: { 'x-api-key': CLAUDE_API_KEY },
      body: request.body
    });

    await incrementMessageCount(deviceId);
    return response;
  }
}
```

---

## Key Takeaways from Research

1. **50 free messages is the magic number** — Raycast, Cursor, and others found this is enough to hook users without breaking the bank

2. **BYOK is expected in AI-native apps** — Power users prefer it for cost control and privacy

3. **Hybrid models win** — Best apps offer both credits AND BYOK, letting users choose

4. **Time limits create urgency** — But can feel punitive if too short

5. **Demo data is table stakes** — Users expect to see the app working before adding their own data

6. **API costs are dropping** — Claude Sonnet is very affordable, 50 trial messages costs pennies

---

## Sources

- [Moesif: Implementing a Freemium Model for API Monetization](https://www.moesif.com/blog/api-monetization/Implementing-a-Freemium-Model-for-API-Monetization/)
- [Warp: Bring Your Own API Key](https://docs.warp.dev/support-and-billing/plans-and-pricing/bring-your-own-api-key)
- [Warp: New Pricing with BYOK](https://www.warp.dev/blog/warp-new-pricing-flexibility-byok)
- [Rilna: What is BYOK?](https://www.rilna.net/blog/bring-your-own-api-key-byok-tools-guide-examples)
- [BYOK.tech](https://www.byok.tech/)
- [CodeGPT: BYOK](https://www.codegpt.co/bring-your-own-api-key)
- [Raycast Pricing](https://www.raycast.com/pricing)
- [Cursor Pricing](https://cursor.com/pricing)
- [FlexPrice: Cursor Pricing Guide](https://flexprice.io/blog/cursor-pricing-guide)
- [Apps.Deals: macOS AI Clients Compared](https://blog.apps.deals/2025-04-28-macos-ai-clients-comparison)
- [BoltAI Review](https://skywork.ai/skypage/en/BoltAI-Review-(2025):-The-macOS-AI-Assistant-That-Actually-Boosts-Productivity/1976172131466801152)
- [TypingMind Pricing](https://www.typingmind.com/buy)
- [AIProxy](https://www.aiproxy.com/)
- [GitHub: AI Proxy Server](https://github.com/SucceedAI/ai-proxy-server)
- [ChatGPT App Monetization Models](https://www.c-sharpcorner.com/article/chatgpt-app-monetization-models-what-developers-need-to-know/)
- [OpenAI: Monetization](https://developers.openai.com/apps-sdk/build/monetization/)
- [eesel.ai: Free AI API Guide](https://www.eesel.ai/blog/free-ai-api)

---

*Research compiled: January 2026*
