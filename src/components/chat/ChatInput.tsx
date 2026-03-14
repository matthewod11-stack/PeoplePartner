import { useState, useRef, useEffect, useCallback, forwardRef, useImperativeHandle } from 'react';

interface ChatInputProps {
  /** Callback when user submits a message (called with trimmed text) */
  onSubmit: (message: string) => void;
  /** Disables input and submit button */
  disabled?: boolean;
  /** Shows offline state styling and disables submit */
  isOffline?: boolean;
  /** Placeholder text */
  placeholder?: string;
  /** Auto-focus on mount */
  autoFocus?: boolean;
}

/** Handle exposed via ref for external focus control */
export interface ChatInputHandle {
  focus: () => void;
}

const MAX_HEIGHT = 200;

export const ChatInput = forwardRef<ChatInputHandle, ChatInputProps>(function ChatInput(
  {
    onSubmit,
    disabled = false,
    isOffline = false,
    placeholder = 'Ask a question...',
    autoFocus = true,
  },
  ref
) {
  const [message, setMessage] = useState('');
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  // Expose focus method via ref
  useImperativeHandle(ref, () => ({
    focus: () => textareaRef.current?.focus(),
  }), []);

  // Combine disabled states - offline also disables submit
  const isInputDisabled = disabled;
  const isSubmitDisabled = disabled || isOffline || !message.trim();

  // Dynamic placeholder for offline state
  const effectivePlaceholder = isOffline
    ? "You're offline. Chat is available when connected."
    : placeholder;

  // Auto-resize textarea based on content
  const adjustHeight = useCallback(() => {
    const textarea = textareaRef.current;
    if (textarea) {
      textarea.style.height = 'auto';
      const newHeight = Math.min(textarea.scrollHeight, MAX_HEIGHT);
      textarea.style.height = `${newHeight}px`;
    }
  }, []);

  // Adjust height when message changes
  useEffect(() => {
    adjustHeight();
  }, [message, adjustHeight]);

  // Auto-focus on mount
  useEffect(() => {
    if (autoFocus && textareaRef.current) {
      textareaRef.current.focus();
    }
  }, [autoFocus]);

  const handleSubmit = useCallback(() => {
    const trimmed = message.trim();
    if (trimmed && !disabled && !isOffline) {
      onSubmit(trimmed);
      setMessage('');
    }
  }, [message, disabled, isOffline, onSubmit]);

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    // Enter without Shift = submit
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSubmit();
    }
    // Shift+Enter allows newline (default behavior)
  };

  return (
    <div className="py-4">
      <div
        className={`
          flex items-end gap-3
          px-4 py-3
          bg-white
          border ${isOffline ? 'border-amber-300' : 'border-stone-200'}
          rounded-xl
          shadow-sm
          ${!isOffline && 'focus-within:border-primary-300 focus-within:ring-2 focus-within:ring-primary-100'}
          transition-all duration-200
          ${isInputDisabled ? 'opacity-60' : ''}
        `}
      >
        {/* Offline indicator icon */}
        {isOffline && (
          <div className="flex-shrink-0 text-amber-500 self-center">
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
                d="M18.364 5.636a9 9 0 010 12.728m0 0l-2.829-2.829m2.829 2.829L21 21M15.536 8.464a5 5 0 010 7.072m0 0l-2.829-2.829m-4.243 2.829a4.978 4.978 0 01-1.414-2.83m-1.414 5.658a9 9 0 01-2.167-9.238m7.824 2.167a1 1 0 111.414 1.414m-1.414-1.414L3 3m8.293 8.293l1.414 1.414"
              />
            </svg>
          </div>
        )}
        <textarea
          ref={textareaRef}
          value={message}
          onChange={(e) => setMessage(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder={effectivePlaceholder}
          disabled={isInputDisabled}
          rows={1}
          aria-label="Message input"
          className={`
            flex-1
            bg-transparent
            text-stone-700
            ${isOffline ? 'placeholder:text-amber-500' : 'placeholder:text-stone-400'}
            focus:outline-none
            resize-none
            min-h-[24px]
            max-h-[200px]
            leading-6
            ${isInputDisabled ? 'cursor-not-allowed' : ''}
          `}
        />
        <button
          type="button"
          onClick={handleSubmit}
          disabled={isSubmitDisabled}
          aria-label="Send message"
          className={`
            w-9 h-9
            flex-shrink-0
            flex items-center justify-center
            rounded-lg
            transition-all duration-200
            ${
              isSubmitDisabled
                ? 'bg-stone-200 text-stone-400 cursor-not-allowed'
                : 'bg-primary-500 hover:bg-primary-600 text-white shadow-sm hover:shadow-md hover:brightness-110 active:brightness-95'
            }
          `}
        >
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
              d="M6 12L3.269 3.126A59.768 59.768 0 0121.485 12 59.77 59.77 0 013.27 20.876L5.999 12zm0 0h7.5"
            />
          </svg>
        </button>
      </div>
    </div>
  );
});

export default ChatInput;
