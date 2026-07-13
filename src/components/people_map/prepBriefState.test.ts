// Prep-brief state machine (FHR-110): pure reducer tests.
import { describe, expect, it } from 'vitest';
import type { PrepBrief } from '../../lib/tauri-commands';
import {
  initialPrepBriefState,
  isRegenerateSuggested,
  prepBriefReducer,
} from './prepBriefState';

const brief: PrepBrief = {
  employeeId: 'emp-1',
  facts: [{ text: 'Led the gateway migration.', citationId: 'C7' }],
  threads: [
    {
      anchorCitationId: 'C7',
      anchorFact: 'Led the gateway migration.',
      question: 'What made the cutover smooth?',
    },
  ],
};

describe('prepBriefReducer', () => {
  it('starts idle and moves to loading on generate', () => {
    expect(initialPrepBriefState.kind).toBe('idle');
    const next = prepBriefReducer(initialPrepBriefState, { type: 'generate' });
    expect(next.kind).toBe('loading');
  });

  it('loading -> ready on success', () => {
    const next = prepBriefReducer(
      { kind: 'loading' },
      { type: 'succeeded', brief }
    );
    expect(next).toEqual({ kind: 'ready', brief });
  });

  it('loading -> error on failure', () => {
    const next = prepBriefReducer(
      { kind: 'loading' },
      { type: 'failed', message: 'boom' }
    );
    expect(next).toEqual({ kind: 'error', message: 'boom' });
  });

  it('ignores stale results after close', () => {
    // close (idle), then a stale success/failure lands — state must not move.
    const closed = prepBriefReducer({ kind: 'loading' }, { type: 'closed' });
    expect(closed.kind).toBe('idle');
    expect(prepBriefReducer(closed, { type: 'succeeded', brief }).kind).toBe(
      'idle'
    );
    expect(
      prepBriefReducer(closed, { type: 'failed', message: 'late' }).kind
    ).toBe('idle');
  });

  it('regenerate from ready goes back to loading', () => {
    const next = prepBriefReducer(
      { kind: 'ready', brief },
      { type: 'generate' }
    );
    expect(next.kind).toBe('loading');
  });
});

describe('isRegenerateSuggested', () => {
  it('detects the NotGrounded backend message', () => {
    expect(
      isRegenerateSuggested(
        'brief cited nothing from the record (2 phantom citations) — regenerate'
      )
    ).toBe(true);
    expect(isRegenerateSuggested('API request failed: HTTP 500')).toBe(false);
  });
});
