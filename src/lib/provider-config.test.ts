import { describe, it, expect } from 'vitest';
import { PROVIDER_META, PROVIDER_ORDER } from './provider-config';

// Invariants for the provider metadata table. These guard against a new
// provider being half-added (order list and meta map drifting apart) or a
// key-prefix hint that contradicts the validation in api-key-errors.ts.

const EXPECTED_PREFIX: Record<string, string> = {
  anthropic: 'sk-ant-',
  openai: 'sk-',
  gemini: 'AIzaSy',
};

describe('PROVIDER_ORDER ↔ PROVIDER_META', () => {
  it('every ordered id has a metadata entry and vice-versa', () => {
    expect([...PROVIDER_ORDER].sort()).toEqual(Object.keys(PROVIDER_META).sort());
  });

  it('order is anthropic, openai, gemini', () => {
    expect(PROVIDER_ORDER).toEqual(['anthropic', 'openai', 'gemini']);
  });
});

describe('each provider entry is complete', () => {
  it.each(Object.entries(PROVIDER_META))('%s has all required non-empty fields', (_id, meta) => {
    for (const field of [
      'displayName',
      'modelName',
      'description',
      'consoleUrl',
      'keysUrl',
      'keyPrefixHint',
    ] as const) {
      expect(meta[field], field).toBeTruthy();
    }
    expect(meta.consoleUrl).toMatch(/^https:\/\//);
    expect(meta.keysUrl).toMatch(/^https:\/\//);
    expect(meta.setupSteps.signup).toBeTruthy();
    expect(meta.setupSteps.billing).toBeTruthy();
    expect(meta.setupSteps.createKey).toBeTruthy();
  });

  it('key-prefix hints agree with api-key-errors validation', () => {
    for (const [id, prefix] of Object.entries(EXPECTED_PREFIX)) {
      expect(PROVIDER_META[id].keyPrefixHint.startsWith(prefix)).toBe(true);
    }
  });
});
