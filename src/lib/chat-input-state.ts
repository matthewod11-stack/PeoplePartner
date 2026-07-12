// People Partner - Chat input button-state logic
// Extracted as a pure function so the send/stop decision is unit-testable
// without rendering the component (#147).

/** Which affordance the chat input should present. */
export type ChatInputMode =
  /** A response is streaming and can be cancelled — show a Stop button. */
  | 'stop'
  /** Ready to send — show an enabled Send button. */
  | 'send'
  /** Nothing to send yet (empty/disabled/offline) — show a disabled Send button. */
  | 'send-disabled';

export interface ChatInputStateArgs {
  /** A response is currently streaming in. */
  isStreaming: boolean;
  /** A stop handler is wired up (Stop is only meaningful when it is). */
  canStop: boolean;
  /** The trimmed input has content. */
  hasText: boolean;
  /** External disable (loading, at message limit, etc.). */
  disabled: boolean;
  /** Offline — sending is unavailable. */
  isOffline: boolean;
}

/**
 * Decide which button the chat input renders.
 *
 * Stop wins while a cancellable stream is in flight; otherwise the input is a
 * Send button, enabled only when there is text and no external block. Keeping
 * this pure means the streaming-cancel wiring in #147 is covered by fast unit
 * tests rather than only through the full component.
 */
export function resolveChatInputMode(args: ChatInputStateArgs): ChatInputMode {
  const { isStreaming, canStop, hasText, disabled, isOffline } = args;

  if (isStreaming && canStop) {
    return 'stop';
  }

  if (disabled || isOffline || !hasText) {
    return 'send-disabled';
  }

  return 'send';
}
