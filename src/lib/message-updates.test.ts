import { describe, it, expect } from 'vitest';
import {
  appendChunk,
  setMessageError,
  setMessageVerification,
  finalizeCancelledMessage,
  toApiMessages,
  resolveRedaction,
} from './message-updates';
import type { Message, ChatError, VerificationResult } from './types';
import type { RedactionResult } from './tauri-commands';

const msg = (id: string, over: Partial<Message> = {}): Message => ({
  id,
  role: 'assistant',
  content: '',
  timestamp: '2026-07-07T00:00:00.000Z',
  ...over,
});

const ERR: ChatError = {
  type: 'network_error',
  message: 'Connection Error',
  details: '...',
  retryable: true,
};

const VERIFICATION: VerificationResult = {
  is_aggregate_query: true,
  claims: [],
  overall_status: 'Verified',
  sql_query: null,
};

describe('appendChunk', () => {
  it('appends to the matching message only', () => {
    const before = [msg('u', { role: 'user', content: 'hi' }), msg('a', { content: 'He' })];
    const after = appendChunk(before, 'a', 'llo');
    expect(after[1].content).toBe('Hello');
    expect(after[0].content).toBe('hi');
  });

  it('is immutable — new array and new object for the matched message', () => {
    const before = [msg('a', { content: 'x' })];
    const after = appendChunk(before, 'a', 'y');
    expect(after).not.toBe(before);
    expect(after[0]).not.toBe(before[0]);
    expect(before[0].content).toBe('x'); // original untouched
  });

  it('leaves unmatched messages by reference', () => {
    const other = msg('u', { role: 'user', content: 'q' });
    const after = appendChunk([other, msg('a')], 'a', 'z');
    expect(after[0]).toBe(other);
  });

  it('no matching id → content unchanged', () => {
    const before = [msg('a', { content: 'keep' })];
    expect(appendChunk(before, 'nope', '!')[0].content).toBe('keep');
  });
});

describe('setMessageError', () => {
  it('clears content and attaches the error on the matched message', () => {
    const after = setMessageError([msg('a', { content: 'half streamed' })], 'a', ERR);
    expect(after[0].content).toBe('');
    expect(after[0].error).toBe(ERR);
  });

  it('does not touch other messages', () => {
    const keep = msg('u', { role: 'user', content: 'q' });
    const after = setMessageError([keep, msg('a', { content: 'x' })], 'a', ERR);
    expect(after[0]).toBe(keep);
    expect(after[0].error).toBeUndefined();
  });
});

describe('finalizeCancelledMessage', () => {
  it('drops the placeholder when nothing had streamed yet', () => {
    const after = finalizeCancelledMessage([msg('u', { role: 'user', content: 'q' }), msg('a')], 'a');
    expect(after).toHaveLength(1);
    expect(after[0].id).toBe('u');
  });

  it('keeps the partial response when text had already streamed', () => {
    const after = finalizeCancelledMessage([msg('a', { content: 'partial ans' })], 'a');
    expect(after).toHaveLength(1);
    expect(after[0].content).toBe('partial ans');
  });

  it('never drops an empty message belonging to a different stream', () => {
    const other = msg('b');
    const after = finalizeCancelledMessage([other, msg('a')], 'a');
    expect(after).toEqual([other]);
  });

  it('is immutable — the original array is untouched', () => {
    const before = [msg('a')];
    const after = finalizeCancelledMessage(before, 'a');
    expect(after).not.toBe(before);
    expect(before).toHaveLength(1);
  });
});

describe('setMessageVerification', () => {
  it('attaches verification to the matched message', () => {
    const after = setMessageVerification([msg('a')], 'a', VERIFICATION);
    expect(after[0].verification).toBe(VERIFICATION);
  });
});

describe('toApiMessages', () => {
  it('drops the trailing (empty assistant placeholder) message', () => {
    const list = [
      msg('u', { role: 'user', content: 'question' }),
      msg('a', { content: '' }), // placeholder being streamed into
    ];
    expect(toApiMessages(list)).toEqual([{ role: 'user', content: 'question' }]);
  });

  it('maps only role + content (strips id/timestamp/error)', () => {
    const list = [msg('u', { role: 'user', content: 'a' }), msg('x', { content: 'b' }), msg('y')];
    expect(toApiMessages(list)).toEqual([
      { role: 'user', content: 'a' },
      { role: 'assistant', content: 'b' },
    ]);
  });

  it('single-element list → empty (nothing precedes the placeholder)', () => {
    expect(toApiMessages([msg('a')])).toEqual([]);
  });
});

describe('resolveRedaction', () => {
  const scan = (over: Partial<RedactionResult>): RedactionResult => ({
    redacted_text: '',
    matches: [],
    had_pii: false,
    summary: null,
    ...over,
  });

  it('PII found → sends redacted text and surfaces the summary', () => {
    const r = resolveRedaction('my ssn is 123-45-6789', scan({
      had_pii: true,
      redacted_text: 'my ssn is [SSN]',
      summary: 'Redacted: 1 SSN',
    }));
    expect(r.content).toBe('my ssn is [SSN]');
    expect(r.notification).toBe('Redacted: 1 SSN');
  });

  it('no PII → original passes through, no notification', () => {
    const r = resolveRedaction('hello team', scan({ had_pii: false }));
    expect(r.content).toBe('hello team');
    expect(r.notification).toBeNull();
  });
});
