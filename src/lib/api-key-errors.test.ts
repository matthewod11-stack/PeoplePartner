import { describe, it, expect } from 'vitest';
import { getApiKeyErrorHint, getStorageErrorMessage } from './api-key-errors';

// Characterization tests for provider-aware API-key hints (#107 onboarding).
// The value here is the cross-provider mismatch detection: pasting an
// Anthropic key into the OpenAI field should say so, not silently fail later.

describe('getApiKeyErrorHint — empty & valid keys return null', () => {
  it('empty string → null (no hint while the field is blank)', () => {
    expect(getApiKeyErrorHint('')).toBeNull();
    expect(getApiKeyErrorHint('', 'openai')).toBeNull();
  });

  it('a well-formed key for its provider → null', () => {
    expect(getApiKeyErrorHint('sk-ant-' + 'a'.repeat(40))).toBeNull(); // anthropic
    expect(getApiKeyErrorHint('sk-proj-abc123', 'openai')).toBeNull();
    expect(getApiKeyErrorHint('AIzaSyABCDEF', 'gemini')).toBeNull();
  });
});

describe('getApiKeyErrorHint — Anthropic (default/legacy)', () => {
  it('flags an OpenAI-shaped key', () => {
    expect(getApiKeyErrorHint('sk-proj-abc')).toMatch(/Anthropic key/);
  });
  it('flags a Gemini-shaped key', () => {
    expect(getApiKeyErrorHint('AIzaSyABC')).toMatch(/Gemini/);
  });
  it('flags a key with no recognizable prefix', () => {
    expect(getApiKeyErrorHint('totally-wrong')).toMatch(/sk-ant-/);
  });
  it('flags a correctly-prefixed but too-short key', () => {
    expect(getApiKeyErrorHint('sk-ant-short')).toMatch(/incomplete/);
  });
});

describe('getApiKeyErrorHint — OpenAI', () => {
  it('flags an Anthropic key pasted into the OpenAI field', () => {
    expect(getApiKeyErrorHint('sk-ant-abc', 'openai')).toMatch(/Anthropic key/);
  });
  it('flags a Gemini key', () => {
    expect(getApiKeyErrorHint('AIzaSyABC', 'openai')).toMatch(/Gemini/);
  });
  it('flags a non-sk key', () => {
    expect(getApiKeyErrorHint('nope', 'openai')).toMatch(/OpenAI keys start with 'sk-'/);
  });
});

describe('getApiKeyErrorHint — Gemini', () => {
  it('flags an OpenAI/Anthropic key', () => {
    expect(getApiKeyErrorHint('sk-ant-abc', 'gemini')).toMatch(/Gemini key/);
    expect(getApiKeyErrorHint('sk-proj-abc', 'gemini')).toMatch(/Gemini key/);
  });
  it('flags a non-AIzaSy key', () => {
    expect(getApiKeyErrorHint('nope', 'gemini')).toMatch(/AIzaSy/);
  });
});

describe('getStorageErrorMessage', () => {
  it('maps permission/access failures', () => {
    expect(getStorageErrorMessage('permission denied')).toMatch(/permission to store data/);
    expect(getStorageErrorMessage('access error')).toMatch(/permission to store data/);
  });
  it('maps disk/write failures', () => {
    expect(getStorageErrorMessage('storage full')).toMatch(/disk or storage/);
    expect(getStorageErrorMessage('write failed')).toMatch(/disk or storage/);
  });
  it('falls back to a generic retry message', () => {
    expect(getStorageErrorMessage('mystery')).toMatch(/Failed to save API key/);
  });
});
