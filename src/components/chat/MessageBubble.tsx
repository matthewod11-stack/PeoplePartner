/**
 * MessageBubble Component
 *
 * Displays a single chat message with appropriate styling for user or assistant messages.
 * Follows the HR Command Center "Warm Editorial" design aesthetic.
 *
 * Assistant messages are rendered as Markdown (supporting bold, lists, code, etc.)
 * User messages are rendered as plain text to preserve what they typed.
 *
 * V2.1.4: Now supports verification badges for aggregate query responses.
 * V2.3.2: Now supports chart visualization for analytics queries.
 */

import { memo } from 'react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import rehypeSanitize from 'rehype-sanitize';
import { VerificationBadge } from './VerificationBadge';
import { AnalyticsChart } from '../analytics';
import type { VerificationResult } from '../../lib/types';
import type { ChartData, AnalyticsRequest } from '../../lib/analytics-types';

const MARKDOWN_REMARK_PLUGINS = [remarkGfm];
const MARKDOWN_REHYPE_PLUGINS = [rehypeSanitize];

/**
 * Formats an ISO timestamp string to a user-friendly time display
 * @param isoString - ISO 8601 timestamp
 * @returns Formatted time string (e.g., "2:34 PM")
 */
function formatTime(isoString: string): string {
  try {
    const date = new Date(isoString);
    return date.toLocaleTimeString('en-US', {
      hour: 'numeric',
      minute: '2-digit',
      hour12: true,
    });
  } catch {
    return '';
  }
}

interface MessageBubbleProps {
  /** The message content to display */
  content: string;
  /** Whether this is a user or assistant message */
  role: 'user' | 'assistant';
  /** ISO timestamp for the message */
  timestamp?: string;
  /** Whether to show the timestamp (defaults to true) */
  showTimestamp?: boolean;
  /** V2.1.4: Verification result for aggregate queries */
  verification?: VerificationResult;
  /** V2.3.2: Chart data for analytics visualization */
  chartData?: ChartData;
  /** V2.3.2h: Analytics request for pinning to insight canvas */
  analyticsRequest?: AnalyticsRequest;
  /** V2.3.2h: Message ID for pinning to insight canvas */
  messageId?: string;
  /** Render as plain text when streaming to avoid markdown re-parse churn */
  renderAsPlainText?: boolean;
}

export const MessageBubble = memo(function MessageBubble({
  content,
  role,
  timestamp,
  showTimestamp = true,
  verification,
  chartData,
  analyticsRequest,
  messageId,
  renderAsPlainText = false,
}: MessageBubbleProps) {
  const isUser = role === 'user';
  const formattedTime = timestamp ? formatTime(timestamp) : null;

  return (
    <div
      className={`
        flex flex-col
        ${isUser ? 'items-end' : 'items-start'}
      `}
      role="article"
      aria-label={`${isUser ? 'Your' : 'Assistant'} message`}
    >
      <div
        className={`
          px-4 py-3
          rounded-xl
          max-w-[80%]
          ${isUser
            ? 'bg-primary-500 text-white'
            : 'bg-stone-100 text-stone-900'
          }
        `}
      >
        {isUser || renderAsPlainText ? (
          // User messages: plain text preserves exactly what they typed
          <p className="text-base leading-relaxed whitespace-pre-wrap break-words">
            {content || '\u00A0'}
          </p>
        ) : (
          // Assistant messages: render as Markdown
          <div className="prose prose-stone prose-sm max-w-none break-words
            prose-p:my-2 prose-p:leading-relaxed
            prose-headings:font-semibold prose-headings:text-stone-900
            prose-h1:text-lg prose-h2:text-base prose-h3:text-sm
            prose-ul:my-2 prose-ol:my-2 prose-li:my-0.5
            prose-code:bg-stone-200 prose-code:px-1 prose-code:py-0.5 prose-code:rounded prose-code:text-sm prose-code:font-mono prose-code:before:content-none prose-code:after:content-none
            prose-pre:bg-stone-800 prose-pre:text-stone-100 prose-pre:rounded-lg prose-pre:my-2
            prose-a:text-primary-600 prose-a:underline hover:prose-a:text-primary-700
            prose-strong:font-semibold prose-strong:text-stone-900
            prose-blockquote:border-l-primary-400 prose-blockquote:text-stone-600
          ">
            <ReactMarkdown
              remarkPlugins={MARKDOWN_REMARK_PLUGINS}
              rehypePlugins={MARKDOWN_REHYPE_PLUGINS}
            >
              {content || '\u00A0'}
            </ReactMarkdown>
          </div>
        )}

        {/* V2.1.4: Verification badge for aggregate queries */}
        {!isUser && verification && (
          <VerificationBadge verification={verification} />
        )}

        {showTimestamp && formattedTime && (
          <span
            className={`
              block text-right text-xs mt-2
              ${isUser ? 'text-white/70' : 'text-stone-500'}
            `}
            aria-label={`Sent at ${formattedTime}`}
          >
            {formattedTime}
          </span>
        )}
      </div>

      {/* V2.3.2: Analytics chart visualization (rendered outside bubble for wider display) */}
      {!isUser && chartData && (
        <div className="w-full max-w-[90%] mt-2">
          <AnalyticsChart
            data={chartData}
            analyticsRequest={analyticsRequest}
            messageId={messageId}
          />
        </div>
      )}
    </div>
  );
});

export default MessageBubble;
