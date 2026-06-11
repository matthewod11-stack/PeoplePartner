import type { ChatError } from '../../lib/types';

interface ErrorMessageProps {
  error: ChatError;
  timestamp?: string;
  onRetry?: () => void;
  onCopyMessage?: () => void;
  /** Opens the Settings panel. Rendered for error types the user fixes in
   *  Settings (key/billing) — a Retry button can't fix those (#110). */
  onOpenSettings?: () => void;
}

/** Error types whose remedy lives in Settings (provider key / billing). */
const SETTINGS_FIXABLE_TYPES = new Set(['no_api_key', 'auth_error', 'billing']);

function formatTime(timestamp: string): string {
  try {
    return new Date(timestamp).toLocaleTimeString([], {
      hour: 'numeric',
      minute: '2-digit',
    });
  } catch {
    return '';
  }
}

export function ErrorMessage({
  error,
  timestamp,
  onRetry,
  onCopyMessage,
  onOpenSettings,
}: ErrorMessageProps) {
  const showRetry = error.retryable && onRetry;
  const showCopy = error.originalContent && onCopyMessage;
  const showSettings = SETTINGS_FIXABLE_TYPES.has(error.type) && onOpenSettings;

  return (
    <div className="flex items-start" role="alert" aria-live="polite">
      <div
        className="
          max-w-[80%]
          px-4 py-3
          bg-red-50
          border border-red-200
          rounded-xl
          shadow-sm
        "
      >
        {/* Header with error icon and title */}
        <div className="flex items-center gap-2 mb-2">
          <div className="flex-shrink-0 text-red-500">
            <svg
              className="w-5 h-5"
              fill="none"
              viewBox="0 0 24 24"
              stroke="currentColor"
              strokeWidth={2}
            >
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z"
              />
            </svg>
          </div>
          <span className="font-medium text-red-800">{error.message}</span>
        </div>

        {/* Error details */}
        <p className="text-sm text-red-700 mb-3">{error.details}</p>

        {/* Action buttons and timestamp */}
        <div className="flex items-center justify-between gap-3">
          <div className="flex items-center gap-2">
            {showRetry && (
              <button
                onClick={onRetry}
                className="
                  inline-flex items-center gap-1.5
                  px-3 py-1.5
                  text-sm font-medium
                  text-red-700
                  bg-red-100
                  hover:bg-red-200
                  rounded-lg
                  transition-colors
                "
                aria-label="Retry sending message"
              >
                <svg
                  className="w-4 h-4"
                  fill="none"
                  viewBox="0 0 24 24"
                  stroke="currentColor"
                  strokeWidth={2}
                >
                  <path
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15"
                  />
                </svg>
                Retry
              </button>
            )}
            {showSettings && (
              <button
                onClick={onOpenSettings}
                className="
                  inline-flex items-center gap-1.5
                  px-3 py-1.5
                  text-sm font-medium
                  text-red-700
                  bg-red-100
                  hover:bg-red-200
                  rounded-lg
                  transition-colors
                "
                aria-label="Open Settings to fix this"
              >
                <svg
                  className="w-4 h-4"
                  fill="none"
                  viewBox="0 0 24 24"
                  stroke="currentColor"
                  strokeWidth={2}
                >
                  <path
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z"
                  />
                  <path
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    d="M15 12a3 3 0 11-6 0 3 3 0 016 0z"
                  />
                </svg>
                Open Settings
              </button>
            )}
            {showCopy && (
              <button
                onClick={onCopyMessage}
                className="
                  inline-flex items-center gap-1.5
                  px-3 py-1.5
                  text-sm font-medium
                  text-stone-600
                  bg-stone-100
                  hover:bg-stone-200
                  rounded-lg
                  transition-colors
                "
                aria-label="Copy original message to clipboard"
              >
                <svg
                  className="w-4 h-4"
                  fill="none"
                  viewBox="0 0 24 24"
                  stroke="currentColor"
                  strokeWidth={2}
                >
                  <path
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z"
                  />
                </svg>
                Copy Message
              </button>
            )}
          </div>

          {timestamp && (
            <span className="text-xs text-red-400">{formatTime(timestamp)}</span>
          )}
        </div>
      </div>
    </div>
  );
}
