/**
 * PIINotification Component
 *
 * Two variants:
 * - 'info' (default): amber notification when PII was auto-redacted.
 *   Auto-dismisses after `duration` ms.
 * - 'error': red notification when the PII scanner failed and the message
 *   was blocked (fail-closed). Persistent; user must dismiss manually.
 *
 * Design spec (from HR-Command-Center-Design-Architecture.md):
 * - Slide in from top (200ms)
 */

import { useEffect, useState, useCallback } from 'react';

type PIINotificationVariant = 'info' | 'error';

interface PIINotificationProps {
  /** Summary text (e.g., "Redacted: 1 SSN, 2 credit cards" or scan-failed message) */
  summary: string | null;
  /** Callback when notification is dismissed */
  onDismiss: () => void;
  /** Duration before auto-dismiss in ms. Ignored for 'error' variant. (default: 3000) */
  duration?: number;
  /** 'info' (amber, auto-dismiss) or 'error' (red, persistent). Defaults to 'info'. */
  variant?: PIINotificationVariant;
}

export function PIINotification({
  summary,
  onDismiss,
  duration = 3000,
  variant = 'info',
}: PIINotificationProps) {
  const [isVisible, setIsVisible] = useState(false);
  const [isLeaving, setIsLeaving] = useState(false);

  const handleDismiss = useCallback(() => {
    setIsLeaving(true);
    // Wait for animation to complete before calling onDismiss
    setTimeout(() => {
      setIsVisible(false);
      setIsLeaving(false);
      onDismiss();
    }, 200);
  }, [onDismiss]);

  useEffect(() => {
    if (summary) {
      setIsVisible(true);
      setIsLeaving(false);

      // Errors are fail-closed signals — they persist until the user
      // acknowledges them. Info notifications auto-dismiss.
      if (variant === 'error') {
        return;
      }

      const timer = setTimeout(handleDismiss, duration);
      return () => clearTimeout(timer);
    }
  }, [summary, duration, handleDismiss, variant]);

  if (!isVisible || !summary) {
    return null;
  }

  const isError = variant === 'error';
  const containerClasses = isError
    ? 'bg-red-50 border-red-300'
    : 'bg-amber-50 border-amber-200';
  const iconClasses = isError ? 'text-red-600' : 'text-amber-600';
  const textClasses = isError ? 'text-red-800' : 'text-amber-800';
  const dismissClasses = isError
    ? 'text-red-500 hover:text-red-700 hover:bg-red-100'
    : 'text-amber-500 hover:text-amber-700 hover:bg-amber-100';

  return (
    <div
      className={`
        fixed top-4 left-1/2 z-50
        flex items-center gap-2
        px-4 py-2.5
        ${containerClasses}
        border
        rounded-lg
        shadow-lg
        text-sm
        transition-all duration-200 ease-out
        ${isLeaving
          ? 'opacity-0 -translate-y-2 -translate-x-1/2'
          : 'opacity-100 translate-y-0 -translate-x-1/2'
        }
      `}
      role="alert"
      aria-live={isError ? 'assertive' : 'polite'}
    >
      {isError ? (
        // Warning triangle for blocked-send error
        <svg
          className={`w-4 h-4 ${iconClasses} flex-shrink-0`}
          fill="none"
          viewBox="0 0 24 24"
          stroke="currentColor"
          strokeWidth={2}
        >
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            d="M12 9v3.75m-9.303 3.376c-.866 1.5.217 3.374 1.948 3.374h14.71c1.732 0 2.813-1.874 1.948-3.374L13.949 3.378c-.866-1.5-3.032-1.5-3.898 0L2.697 16.126zM12 15.75h.007v.008H12v-.008z"
          />
        </svg>
      ) : (
        // Shield icon for successful redaction
        <svg
          className={`w-4 h-4 ${iconClasses} flex-shrink-0`}
          fill="none"
          viewBox="0 0 24 24"
          stroke="currentColor"
          strokeWidth={2}
        >
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            d="M9 12.75L11.25 15 15 9.75m-3-7.036A11.959 11.959 0 013.598 6 11.99 11.99 0 003 9.749c0 5.592 3.824 10.29 9 11.623 5.176-1.332 9-6.03 9-11.622 0-1.31-.21-2.571-.598-3.751h-.152c-3.196 0-6.1-1.248-8.25-3.285z"
          />
        </svg>
      )}

      <span className={`${textClasses} font-medium`}>
        {summary}
      </span>

      <button
        onClick={handleDismiss}
        className={`
          ml-1 p-0.5
          ${dismissClasses}
          rounded
          transition-colors duration-150
        `}
        aria-label="Dismiss notification"
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
            d="M6 18L18 6M6 6l12 12"
          />
        </svg>
      </button>
    </div>
  );
}

export default PIINotification;
