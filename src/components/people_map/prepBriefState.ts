/**
 * Prep-brief state machine (FHR-110).
 *
 * Pure reducer so the loading/ready/error flow is unit-testable without the
 * IPC layer. Briefs are ephemeral (People Map decision 9): closing the modal
 * discards state entirely — there is deliberately no cache and no library.
 */

import type { PrepBrief } from '../../lib/tauri-commands';

export type PrepBriefState =
  | { kind: 'idle' }
  | { kind: 'loading' }
  | { kind: 'ready'; brief: PrepBrief }
  | { kind: 'error'; message: string };

export type PrepBriefEvent =
  | { type: 'generate' }
  | { type: 'succeeded'; brief: PrepBrief }
  | { type: 'failed'; message: string }
  | { type: 'closed' };

export const initialPrepBriefState: PrepBriefState = { kind: 'idle' };

export function prepBriefReducer(
  state: PrepBriefState,
  event: PrepBriefEvent
): PrepBriefState {
  switch (event.type) {
    case 'generate':
      return { kind: 'loading' };
    case 'succeeded':
      // A stale success after close must not resurrect the modal's content.
      return state.kind === 'loading'
        ? { kind: 'ready', brief: event.brief }
        : state;
    case 'failed':
      return state.kind === 'loading'
        ? { kind: 'error', message: event.message }
        : state;
    case 'closed':
      return { kind: 'idle' };
    default:
      return state;
  }
}

/**
 * The backend returns a NotGrounded error when a generation cited nothing
 * real (T7: drop, don't retry — the user regenerates deliberately). Detect
 * it so the UI can phrase the error as "try again" rather than a failure.
 */
export function isRegenerateSuggested(message: string): boolean {
  return message.toLowerCase().includes('regenerate');
}
