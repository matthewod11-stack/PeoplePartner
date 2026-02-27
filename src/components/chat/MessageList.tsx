/**
 * MessageList Component
 *
 * Displays a scrollable list of chat messages with smart spacing based on
 * speaker changes. Shows welcome content with contextual prompt suggestions
 * when no messages exist.
 */

import { useRef, useEffect } from 'react';
import { Message } from '../../lib/types';
import { MessageBubble } from './MessageBubble';
import { ErrorMessage } from './ErrorMessage';
import { TypingIndicator } from './TypingIndicator';
import { PromptSuggestions } from './PromptSuggestions';
import { MondayDigest } from './MondayDigest';
import { usePromptSuggestions } from '../../hooks/usePromptSuggestions';
import { useMondayDigest } from '../../hooks/useMondayDigest';

interface MessageListProps {
  /** Array of messages to display */
  messages: Message[];
  /** Shows typing indicator when true */
  isLoading?: boolean;
  /** Callback when a prompt suggestion is clicked */
  onPromptClick?: (prompt: string) => void;
  /** Callback to retry a failed message */
  onRetry?: (messageId: string) => void;
  /** Callback to copy original message content */
  onCopyMessage?: (content: string) => void;
}

/**
 * Returns appropriate spacing class based on speaker changes
 * Same speaker: 16px (mt-4), Different speaker: 24px (mt-6)
 */
function getMessageSpacing(
  current: Message,
  previous: Message | undefined
): string {
  if (!previous) return '';
  return current.role !== previous.role ? 'mt-6' : 'mt-4';
}

/**
 * Get contextual heading and description based on suggestion context
 */
function getWelcomeText(context: 'empty' | 'employee-selected' | 'general', employeeName: string | null) {
  switch (context) {
    case 'empty':
      return {
        heading: 'Let\'s get started',
        description: 'Import your employee data to unlock HR insights, or ask me anything about HR best practices.',
      };
    case 'employee-selected':
      return {
        heading: `About ${employeeName}`,
        description: `Ask me anything about ${employeeName}—performance, feedback, or draft communications.`,
      };
    case 'general':
    default:
      return {
        heading: 'What can I help with?',
        description: 'Ask me anything about your team—performance reviews, PTO policies, onboarding, or employee questions.',
      };
  }
}

/**
 * Welcome content shown when no messages exist
 * Uses contextual suggestions based on app state
 */
function WelcomeContent({
  onPromptClick,
}: {
  onPromptClick?: (prompt: string) => void;
}) {
  const { suggestions, context, selectedEmployeeName } = usePromptSuggestions();
  const { heading, description } = getWelcomeText(context, selectedEmployeeName);
  const { isVisible: showDigest, isLoading: digestLoading, anniversaries, newHires, dismiss: dismissDigest } = useMondayDigest();

  // Different icon based on context
  const iconPath = context === 'employee-selected'
    ? 'M15.75 6a3.75 3.75 0 11-7.5 0 3.75 3.75 0 017.5 0zM4.501 20.118a7.5 7.5 0 0114.998 0A17.933 17.933 0 0112 21.75c-2.676 0-5.216-.584-7.499-1.632z'
    : 'M8.625 12a.375.375 0 11-.75 0 .375.375 0 01.75 0zm0 0H8.25m4.125 0a.375.375 0 11-.75 0 .375.375 0 01.75 0zm0 0H12m4.125 0a.375.375 0 11-.75 0 .375.375 0 01.75 0zm0 0h-.375M21 12c0 4.556-4.03 8.25-9 8.25a9.764 9.764 0 01-2.555-.337A5.972 5.972 0 015.41 20.97a5.969 5.969 0 01-.474-.065 4.48 4.48 0 00.978-2.025c.09-.457-.133-.901-.467-1.226C3.93 16.178 3 14.189 3 12c0-4.556 4.03-8.25 9-8.25s9 3.694 9 8.25z';

  return (
    <div className="flex-1 flex flex-col justify-center items-center text-center py-12">
      {/* Monday Digest - shows above welcome content */}
      {showDigest && !digestLoading && (
        <MondayDigest
          anniversaries={anniversaries}
          newHires={newHires}
          onDismiss={dismissDigest}
        />
      )}
      {/* Icon */}
      <div
        className="
          w-16 h-16 mb-6
          rounded-2xl
          bg-gradient-to-br from-primary-100 to-primary-50
          flex items-center justify-center
          shadow-sm
        "
      >
        <svg
          className="w-8 h-8 text-primary-500"
          fill="none"
          viewBox="0 0 24 24"
          stroke="currentColor"
          strokeWidth={1.5}
          aria-hidden="true"
        >
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            d={iconPath}
          />
        </svg>
      </div>

      {/* Heading */}
      <h2 className="font-display text-xl font-semibold text-stone-800 mb-2">
        {heading}
      </h2>

      {/* Description */}
      <p className="text-stone-500 max-w-sm mb-8">
        {description}
      </p>

      {/* Contextual prompt suggestions */}
      <PromptSuggestions
        suggestions={suggestions}
        onSelect={(text) => onPromptClick?.(text)}
        variant="welcome"
        className="max-w-md"
        maxSuggestions={4}
      />
    </div>
  );
}

export function MessageList({
  messages,
  isLoading = false,
  onPromptClick,
  onRetry,
  onCopyMessage,
}: MessageListProps) {
  const messagesEndRef = useRef<HTMLDivElement>(null);

  // Auto-scroll to bottom when messages change or loading starts
  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages, isLoading]);

  // Empty state - show welcome content
  if (messages.length === 0) {
    return <WelcomeContent onPromptClick={onPromptClick} />;
  }

  // Messages list
  return (
    <div
      className="flex-1 overflow-y-auto"
      role="log"
      aria-label="Chat messages"
      aria-live="polite"
    >
      <div className="py-4">
        {messages.map((message, index) => (
          <div
            key={message.id}
            className={getMessageSpacing(message, messages[index - 1])}
          >
            {message.error ? (
              <ErrorMessage
                error={message.error}
                timestamp={message.timestamp}
                onRetry={
                  message.error.retryable && onRetry
                    ? () => onRetry(message.id)
                    : undefined
                }
                onCopyMessage={
                  message.error.originalContent && onCopyMessage
                    ? () => onCopyMessage(message.error!.originalContent!)
                    : undefined
                }
              />
            ) : (
              <MessageBubble
                content={message.content}
                role={message.role}
                timestamp={message.timestamp}
                verification={message.verification}
                renderAsPlainText={
                  isLoading &&
                  index === messages.length - 1 &&
                  message.role === 'assistant'
                }
              />
            )}
          </div>
        ))}

        {/* Typing indicator */}
        {isLoading && <TypingIndicator className="mt-4" />}

        {/* Scroll anchor */}
        <div ref={messagesEndRef} />
      </div>
    </div>
  );
}

export default MessageList;
