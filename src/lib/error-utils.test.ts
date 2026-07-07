import { describe, it, expect } from 'vitest';
import { categorizeError } from './error-utils';

// Characterization tests for chat error categorization (#110, #108).
// These lock the pattern table AND its ORDER — the ordering is load-bearing:
// billing must beat rate_limit and the api_error catch-all, or a "Retry"
// button gets offered for an empty credit balance it can never fix.

describe('categorizeError — the eight branches', () => {
  it('no_api_key: missing key is non-retryable', () => {
    const e = categorizeError('API key not configured');
    expect(e.type).toBe('no_api_key');
    expect(e.retryable).toBe(false);
  });

  it('trial_limit: trial exhaustion is non-retryable', () => {
    expect(categorizeError('trial message limit reached').type).toBe('trial_limit');
    expect(categorizeError('trial_limit_reached').type).toBe('trial_limit');
    expect(categorizeError('Please upgrade to continue').type).toBe('trial_limit');
    expect(categorizeError('trial message limit reached').retryable).toBe(false);
  });

  it('auth_error: invalid/authentication key is non-retryable', () => {
    expect(categorizeError('authentication_error').type).toBe('auth_error');
    expect(categorizeError('invalid_api_key').type).toBe('auth_error');
    expect(categorizeError('The Invalid API key you supplied').type).toBe('auth_error');
    expect(categorizeError('authentication_error').retryable).toBe(false);
  });

  it('rate_limit: throttling is retryable', () => {
    expect(categorizeError('rate_limit_error').type).toBe('rate_limit');
    expect(categorizeError('too many requests').type).toBe('rate_limit');
    expect(categorizeError('rate_limit_error').retryable).toBe(true);
  });

  it('network_error: connectivity failures are retryable', () => {
    expect(categorizeError('API request failed').type).toBe('network_error');
    expect(categorizeError('connection refused').type).toBe('network_error');
    expect(categorizeError('request timeout').type).toBe('network_error');
    expect(categorizeError('unable to connect').type).toBe('network_error');
    expect(categorizeError('network unreachable').retryable).toBe(true);
  });

  it('api_error: generic service error catch-all is retryable', () => {
    const e = categorizeError('API returned error: HTTP 500: internal');
    expect(e.type).toBe('api_error');
    expect(e.retryable).toBe(true);
  });

  it('unknown: unmatched text falls back, retryable', () => {
    const e = categorizeError('something totally unrelated happened');
    expect(e.type).toBe('unknown');
    expect(e.retryable).toBe(true);
  });
});

describe('categorizeError — billing (#110) and its precedence', () => {
  it('classifies each provider’s credit/quota exhaustion as billing (non-retryable)', () => {
    // The real wrapped strings from each provider:
    const anthropic =
      'API returned error: HTTP 400: Your credit balance is too low to access the Anthropic API. Please go to Plans & Billing.';
    const openai =
      'API returned error: HTTP 429: You exceeded your current quota, please check your plan and billing details.';
    const gemini = 'API returned error: HTTP 429: Resource has been exhausted (e.g. check quota).';
    const openaiQuotaCode = 'insufficient_quota';

    for (const s of [anthropic, openai, gemini, openaiQuotaCode]) {
      const e = categorizeError(s);
      expect(e.type).toBe('billing');
      expect(e.retryable).toBe(false);
    }
  });

  it('billing beats the api_error catch-all (both patterns match the wrapped string)', () => {
    // "API returned error" ALSO matches api_error — billing must win by order.
    const s = 'API returned error: HTTP 400: Your credit balance is too low.';
    expect(categorizeError(s).type).toBe('billing');
  });

  it('OpenAI "Rate limit reached" is rate_limit, not the catch-all', () => {
    // Regression for the #110 bonus fix: "Rate limit reached" (space, no
    // underscore) previously fell through to api_error/unknown.
    expect(categorizeError('Rate limit reached for gpt-4o').type).toBe('rate_limit');
  });
});

describe('categorizeError — input shapes', () => {
  it('unwraps an Error instance via its message', () => {
    expect(categorizeError(new Error('network unreachable')).type).toBe('network_error');
  });

  it('coerces null/undefined to the unknown fallback', () => {
    expect(categorizeError(null).type).toBe('unknown');
    expect(categorizeError(undefined).type).toBe('unknown');
  });
});
