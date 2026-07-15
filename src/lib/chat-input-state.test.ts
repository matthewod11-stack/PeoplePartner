import { describe, it, expect } from 'vitest';
import { resolveChatInputMode } from './chat-input-state';

// Unit coverage for the send/stop decision extracted in #147. The load-bearing
// behavior: a cancellable in-flight stream shows Stop; everything else is a
// Send button whose enablement follows text + disabled + offline.

const base = {
  isStreaming: false,
  canStop: false,
  hasText: false,
  disabled: false,
  isOffline: false,
};

describe('resolveChatInputMode — streaming shows Stop', () => {
  it('streaming with a stop handler → stop (even with no text)', () => {
    expect(resolveChatInputMode({ ...base, isStreaming: true, canStop: true })).toBe('stop');
  });

  it('streaming with a stop handler → stop, ignoring disabled/offline', () => {
    expect(
      resolveChatInputMode({ ...base, isStreaming: true, canStop: true, disabled: true, isOffline: true })
    ).toBe('stop');
  });

  it('streaming but no stop handler wired → falls through to send rules', () => {
    // No handler means Stop cannot do anything; with no text it is a disabled Send.
    expect(resolveChatInputMode({ ...base, isStreaming: true, canStop: false })).toBe('send-disabled');
  });
});

describe('resolveChatInputMode — not streaming behaves as a Send button', () => {
  it('has text, enabled, online → send', () => {
    expect(resolveChatInputMode({ ...base, hasText: true })).toBe('send');
  });

  it('empty input → send-disabled', () => {
    expect(resolveChatInputMode({ ...base, hasText: false })).toBe('send-disabled');
  });

  it('disabled (e.g. loading / at message limit) → send-disabled even with text', () => {
    expect(resolveChatInputMode({ ...base, hasText: true, disabled: true })).toBe('send-disabled');
  });

  it('offline → send-disabled even with text', () => {
    expect(resolveChatInputMode({ ...base, hasText: true, isOffline: true })).toBe('send-disabled');
  });
});
