// People Partner - Conversation Summary Hook
// Generates and saves summaries for cross-conversation memory

import { useState, useCallback, useRef } from 'react';
import { Message } from '../lib/types';
import { generateConversationSummary, saveConversationSummary } from '../lib/tauri-commands';

/** Minimum number of message exchanges before generating a summary */
const MIN_EXCHANGES_FOR_SUMMARY = 2;

interface UseConversationSummaryOptions {
  /** Callback when summary is generated successfully */
  onSummaryGenerated?: (summary: string) => void;
  /** Callback when summary generation fails */
  onError?: (error: Error) => void;
}

interface UseConversationSummaryReturn {
  /** Whether summary generation is in progress */
  isGenerating: boolean;
  /** Last error from summary generation */
  error: Error | null;
  /** Generate a summary for the current conversation */
  generateSummary: (conversationId: string, messages: Message[]) => Promise<string | null>;
  /** Check if conversation has enough content for a useful summary */
  canGenerateSummary: (messages: Message[]) => boolean;
  /** Reset error state */
  clearError: () => void;
}

/**
 * Hook for managing conversation summary generation
 *
 * Usage:
 * ```tsx
 * const { generateSummary, isGenerating, canGenerateSummary } = useConversationSummary();
 *
 * // When starting a new conversation:
 * if (canGenerateSummary(messages)) {
 *   await generateSummary(conversationId, messages);
 * }
 * ```
 */
export function useConversationSummary(
  options: UseConversationSummaryOptions = {}
): UseConversationSummaryReturn {
  const [isGenerating, setIsGenerating] = useState(false);
  const [error, setError] = useState<Error | null>(null);

  // Track if we've already generated a summary for this conversation
  const generatedIds = useRef<Set<string>>(new Set());

  const canGenerateSummary = useCallback((messages: Message[]): boolean => {
    // Need at least MIN_EXCHANGES_FOR_SUMMARY user messages with responses
    const userMessages = messages.filter(m => m.role === 'user');
    const assistantMessages = messages.filter(m => m.role === 'assistant');

    // Must have complete exchanges (user + assistant pairs)
    const exchanges = Math.min(userMessages.length, assistantMessages.length);
    return exchanges >= MIN_EXCHANGES_FOR_SUMMARY;
  }, []);

  const generateSummary = useCallback(
    async (conversationId: string, messages: Message[]): Promise<string | null> => {
      // Don't regenerate for same conversation
      if (generatedIds.current.has(conversationId)) {
        console.log('Summary already generated for conversation:', conversationId);
        return null;
      }

      // Validate we have enough content
      if (!canGenerateSummary(messages)) {
        console.log('Not enough messages for summary');
        return null;
      }

      setIsGenerating(true);
      setError(null);

      try {
        // Convert messages to the format expected by the backend
        const messagesJson = JSON.stringify(messages);

        // Generate summary using Claude
        const summary = await generateConversationSummary(messagesJson);

        // Save to database
        await saveConversationSummary(conversationId, summary);

        // Mark as generated
        generatedIds.current.add(conversationId);

        console.log('Summary generated and saved:', summary.substring(0, 100) + '...');

        options.onSummaryGenerated?.(summary);
        return summary;
      } catch (err) {
        const error = err instanceof Error ? err : new Error(String(err));
        setError(error);
        options.onError?.(error);
        console.error('Failed to generate summary:', error);
        return null;
      } finally {
        setIsGenerating(false);
      }
    },
    [canGenerateSummary, options]
  );

  const clearError = useCallback(() => {
    setError(null);
  }, []);

  return {
    isGenerating,
    error,
    generateSummary,
    canGenerateSummary,
    clearError,
  };
}
