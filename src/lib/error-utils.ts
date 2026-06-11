// Error categorization utilities for chat errors
// Parses backend error strings into user-friendly ChatError objects

import type { ChatError, ChatErrorType } from './types';

interface ErrorPattern {
  pattern: RegExp | string;
  type: ChatErrorType;
  message: string;
  details: string;
  retryable: boolean;
}

const ERROR_PATTERNS: ErrorPattern[] = [
  {
    pattern: /API key not configured/i,
    type: 'no_api_key',
    message: 'API Key Required',
    details: "Add your AI provider's API key in Settings to continue.",
    retryable: false,
  },
  {
    pattern: /trial message limit reached|trial_limit_reached|upgrade to continue/i,
    type: 'trial_limit',
    message: 'Trial Limit Reached',
    details: 'You have used all trial messages. Upgrade and add a license key to continue.',
    retryable: false,
  },
  {
    pattern: /authentication_error|invalid.*api.*key|invalid_api_key/i,
    type: 'auth_error',
    message: 'Invalid API Key',
    details: 'Your API key appears to be invalid. Please check your settings.',
    retryable: false,
  },
  {
    // Provider credit/quota exhaustion (#110). Matched against the real
    // wrapped strings ("API returned error: HTTP {n}: {parsed}"):
    //   Anthropic 400 — "Your credit balance is too low to access the
    //                    Anthropic API. Please go to Plans & Billing..."
    //   OpenAI 429    — "You exceeded your current quota, please check your
    //                    plan and billing details." (insufficient_quota)
    //   Gemini 429    — "Resource has been exhausted (e.g. check quota)."
    // Must precede rate_limit and the api_error catch-all: a Retry button
    // can never fix an empty credit balance.
    pattern:
      /credit balance is too low|insufficient_quota|exceeded your current quota|check your plan and billing|resource has been exhausted/i,
    type: 'billing',
    message: 'Provider Out of Credits',
    details:
      'Your AI provider account is out of credits or over its quota. Add credits in ' +
      "your provider's billing console (or switch providers in Settings) to continue — " +
      'your People Partner license is unaffected.',
    retryable: false,
  },
  {
    // "rate.?limit" covers both Anthropic's "rate_limit_error" and OpenAI's
    // "Rate limit reached" phrasing — the latter previously fell through to
    // the generic catch-all (caught while testing #110).
    pattern: /rate.?limit|too many requests/i,
    type: 'rate_limit',
    message: 'Rate Limited',
    details: 'Too many requests. Please wait a moment and try again.',
    retryable: true,
  },
  {
    pattern: /API request failed|connection|timeout|network|unable to connect/i,
    type: 'network_error',
    message: 'Connection Error',
    details: 'Could not connect to the AI service. Please check your internet connection.',
    retryable: true,
  },
  {
    pattern: /API returned error|API error/i,
    type: 'api_error',
    message: 'Service Error',
    details: 'The AI service returned an error. Please try again.',
    retryable: true,
  },
];

/**
 * Categorizes an error into a user-friendly ChatError object.
 * Pattern matches on backend error strings to determine type and messaging.
 */
export function categorizeError(error: unknown): ChatError {
  const errorStr = error instanceof Error ? error.message : String(error);

  for (const pattern of ERROR_PATTERNS) {
    const matches =
      typeof pattern.pattern === 'string'
        ? errorStr.includes(pattern.pattern)
        : pattern.pattern.test(errorStr);

    if (matches) {
      return {
        type: pattern.type,
        message: pattern.message,
        details: pattern.details,
        retryable: pattern.retryable,
      };
    }
  }

  // Default fallback for unknown errors
  return {
    type: 'unknown',
    message: 'Something Went Wrong',
    details: 'An unexpected error occurred. Please try again.',
    retryable: true,
  };
}
