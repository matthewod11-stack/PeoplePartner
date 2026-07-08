// Pure, immutable transforms over the chat Message[] list, plus the outgoing
// PII-redaction decision. Extracted from ConversationContext.sendMessage (#115
// Floor 1) so the streaming update paths — append-chunk, done/verification,
// error, and the fail-closed redaction choice — are unit-testable without
// rendering the provider or mocking the Tauri event stream. The provider still
// owns orchestration (timers, listeners, refs); these are the pure pieces it
// applies via setMessages().

import type { Message, ChatError, VerificationResult } from './types';
import type { ChatMessage, RedactionResult } from './tauri-commands';

/** Append a streamed chunk to the assistant message with the given id. */
export function appendChunk(messages: Message[], id: string, chunk: string): Message[] {
  return messages.map((m) => (m.id === id ? { ...m, content: m.content + chunk } : m));
}

/**
 * Put a message into its error state: content is cleared (a half-streamed
 * response must not be shown next to an error) and the ChatError is attached.
 */
export function setMessageError(messages: Message[], id: string, error: ChatError): Message[] {
  return messages.map((m) => (m.id === id ? { ...m, content: '', error } : m));
}

/** Attach an answer-verification result to a message (V2.1.4). */
export function setMessageVerification(
  messages: Message[],
  id: string,
  verification: VerificationResult
): Message[] {
  return messages.map((m) => (m.id === id ? { ...m, verification } : m));
}

/**
 * Build the provider API history from the UI message list. The trailing
 * message is the just-added empty assistant placeholder and is excluded.
 */
export function toApiMessages(messages: Message[]): ChatMessage[] {
  return messages.slice(0, -1).map((m) => ({ role: m.role, content: m.content }));
}

export interface RedactionResolution {
  /** The content to actually send (redacted when PII was found). */
  content: string;
  /** Notification summary to surface, or null when nothing was redacted. */
  notification: string | null;
}

/**
 * Decide the outgoing message content from a PII scan result. When PII was
 * found, the redacted text is sent and its summary surfaced; otherwise the
 * original content passes through untouched. (The fail-closed path — scan
 * threw, so nothing is sent — stays in the provider's try/catch; this covers
 * the had-pii / clean decision, which is the part with two branches.)
 */
export function resolveRedaction(original: string, result: RedactionResult): RedactionResolution {
  if (result.had_pii) {
    return { content: result.redacted_text, notification: result.summary };
  }
  return { content: original, notification: null };
}
