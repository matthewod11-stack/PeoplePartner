// People Partner - Conversation Card Component
// Displays a conversation in the sidebar list

import type { ConversationListItem } from '../../lib/tauri-commands';

interface ConversationCardProps {
  conversation: ConversationListItem;
  isSelected: boolean;
  onClick: () => void;
  onDelete?: () => void;
}

/**
 * Format a timestamp as relative time (e.g., "2h ago", "Yesterday")
 */
function formatRelativeTime(timestamp: string): string {
  const date = new Date(timestamp);
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffMins = Math.floor(diffMs / (1000 * 60));
  const diffHours = Math.floor(diffMs / (1000 * 60 * 60));
  const diffDays = Math.floor(diffMs / (1000 * 60 * 60 * 24));

  if (diffMins < 1) return 'Just now';
  if (diffMins < 60) return `${diffMins}m ago`;
  if (diffHours < 24) return `${diffHours}h ago`;
  if (diffDays === 1) return 'Yesterday';
  if (diffDays < 7) return `${diffDays}d ago`;

  // Format as date for older conversations
  return date.toLocaleDateString('en-US', {
    month: 'short',
    day: 'numeric',
  });
}

/**
 * Get preview text from conversation
 */
function getPreviewText(conversation: ConversationListItem): string {
  // Prefer summary, fall back to first message preview
  if (conversation.summary) {
    return conversation.summary;
  }
  if (conversation.first_message_preview) {
    return conversation.first_message_preview;
  }
  return 'Empty conversation';
}

export function ConversationCard({
  conversation,
  isSelected,
  onClick,
  onDelete,
}: ConversationCardProps) {
  const title = conversation.title || 'New Conversation';
  const preview = getPreviewText(conversation);
  const timestamp = formatRelativeTime(conversation.updated_at);

  return (
    <button
      onClick={onClick}
      className={`
        w-full p-3 rounded-lg text-left
        group relative
        transition-all duration-200
        ${
          isSelected
            ? 'bg-primary-50 border border-primary-200 shadow-sm'
            : 'bg-white/60 border border-transparent hover:bg-white hover:border-stone-200/60 hover:shadow-sm'
        }
      `}
      aria-pressed={isSelected}
    >
      {/* Title row */}
      <div className="flex items-start justify-between gap-2">
        <p
          className={`
            text-sm font-medium line-clamp-2 flex-1
            ${isSelected ? 'text-primary-800' : 'text-stone-800'}
          `}
          title={title}
        >
          {title}
        </p>
        <span className="text-xs text-stone-500 flex-shrink-0 mt-0.5">
          {timestamp}
        </span>
      </div>

      {/* Preview text */}
      <p className="text-sm text-stone-500 mt-1 line-clamp-2">
        {preview}
      </p>

      {/* Message count */}
      {conversation.message_count > 0 && (
        <div className="flex items-center gap-2 mt-2">
          <span className="text-xs text-stone-500">
            {conversation.message_count} {conversation.message_count === 1 ? 'message' : 'messages'}
          </span>
        </div>
      )}

      {/* Delete button (shows on hover) */}
      {onDelete && (
        <button
          onClick={(e) => {
            e.stopPropagation();
            onDelete();
          }}
          className={`
            absolute top-1 right-1
            w-10 h-10 rounded-md
            flex items-center justify-center
            text-stone-500 hover:text-red-500 hover:bg-red-50
            opacity-0 group-hover:opacity-100
            transition-all duration-200
          `}
          aria-label="Delete conversation"
        >
          <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
            <path strokeLinecap="round" strokeLinejoin="round" d="M14.74 9l-.346 9m-4.788 0L9.26 9m9.968-3.21c.342.052.682.107 1.022.166m-1.022-.165L18.16 19.673a2.25 2.25 0 01-2.244 2.077H8.084a2.25 2.25 0 01-2.244-2.077L4.772 5.79m14.456 0a48.108 48.108 0 00-3.478-.397m-12 .562c.34-.059.68-.114 1.022-.165m0 0a48.11 48.11 0 013.478-.397m7.5 0v-.916c0-1.18-.91-2.164-2.09-2.201a51.964 51.964 0 00-3.32 0c-1.18.037-2.09 1.022-2.09 2.201v.916m7.5 0a48.667 48.667 0 00-7.5 0" />
          </svg>
        </button>
      )}
    </button>
  );
}

export default ConversationCard;
