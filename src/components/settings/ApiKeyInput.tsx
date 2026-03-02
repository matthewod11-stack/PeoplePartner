import { useState, useCallback, useEffect } from 'react';
import {
  storeApiKey, hasApiKey, deleteApiKey, validateApiKeyFormat,
  storeProviderApiKey, hasProviderApiKey, deleteProviderApiKey, validateProviderApiKeyFormat,
} from '../../lib/tauri-commands';
import { getApiKeyErrorHint, getStorageErrorMessage } from '../../lib/api-key-errors';
import { PROVIDER_META } from '../../lib/provider-config';

type ApiKeyStatus = 'idle' | 'validating' | 'saving' | 'saved' | 'error';

interface ApiKeyInputProps {
  /** Callback when API key is successfully saved */
  onSave?: () => void;
  /** Callback when API key is deleted */
  onDelete?: () => void;
  /** Show compact version (for settings panel) */
  compact?: boolean;
  /** Provider ID for multi-provider mode. When undefined, uses legacy Anthropic-only behavior. */
  providerId?: string;
}

export function ApiKeyInput({
  onSave,
  onDelete,
  compact = false,
  providerId,
}: ApiKeyInputProps) {
  const [apiKey, setApiKey] = useState('');
  const [status, setStatus] = useState<ApiKeyStatus>('idle');
  const [hasExistingKey, setHasExistingKey] = useState(false);
  const [errorMessage, setErrorMessage] = useState('');
  const [isValid, setIsValid] = useState(false);

  const meta = providerId ? PROVIDER_META[providerId] : undefined;

  // Check if key already exists on mount (and when providerId changes)
  useEffect(() => {
    const check = providerId ? hasProviderApiKey(providerId) : hasApiKey();
    check
      .then(setHasExistingKey)
      .catch(() => setHasExistingKey(false));
    // Reset input state when provider changes
    setApiKey('');
    setStatus('idle');
    setErrorMessage('');
    setIsValid(false);
  }, [providerId]);

  // Validate format as user types
  useEffect(() => {
    if (!apiKey) {
      setIsValid(false);
      return;
    }

    const timer = setTimeout(async () => {
      try {
        const valid = providerId
          ? await validateProviderApiKeyFormat(providerId, apiKey)
          : await validateApiKeyFormat(apiKey);
        setIsValid(valid);
      } catch {
        setIsValid(false);
      }
    }, 300);

    return () => clearTimeout(timer);
  }, [apiKey, providerId]);

  const handleSave = useCallback(async () => {
    if (!apiKey || !isValid) return;

    setStatus('saving');
    setErrorMessage('');

    try {
      if (providerId) {
        await storeProviderApiKey(providerId, apiKey);
      } else {
        await storeApiKey(apiKey);
      }
      setStatus('saved');
      setHasExistingKey(true);
      setApiKey('');
      onSave?.();

      // Reset status after brief display
      setTimeout(() => setStatus('idle'), 2000);
    } catch (err) {
      setStatus('error');
      const errorStr = err instanceof Error ? err.message : String(err);
      setErrorMessage(getStorageErrorMessage(errorStr));
    }
  }, [apiKey, isValid, onSave, providerId]);

  const handleDelete = useCallback(async () => {
    try {
      if (providerId) {
        await deleteProviderApiKey(providerId);
      } else {
        await deleteApiKey();
      }
      setHasExistingKey(false);
      setStatus('idle');
      onDelete?.();
    } catch (err) {
      setErrorMessage(err instanceof Error ? err.message : 'Failed to delete API key');
    }
  }, [onDelete, providerId]);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && isValid && status === 'idle') {
      handleSave();
    }
  };

  // Dynamic label and placeholder
  const displayName = meta?.displayName ?? 'Anthropic';
  const placeholder = meta?.keyPrefixHint ?? 'sk-ant-...';
  const keysUrl = meta?.keysUrl ?? 'https://console.anthropic.com/settings/keys';

  // Determine visual feedback
  const getBorderColor = () => {
    if (errorMessage) return 'border-red-300 focus-within:border-red-400 focus-within:ring-red-100';
    if (status === 'saved') return 'border-green-300 focus-within:border-green-400 focus-within:ring-green-100';
    if (isValid && apiKey) return 'border-green-300 focus-within:border-green-400 focus-within:ring-green-100';
    return 'border-stone-200 focus-within:border-primary-300 focus-within:ring-primary-100';
  };

  if (hasExistingKey && status !== 'saved') {
    return (
      <div className={compact ? '' : 'py-4'}>
        <div className="flex items-center justify-between gap-4 p-4 bg-green-50 border border-green-200 rounded-xl">
          <div className="flex items-center gap-3">
            <div className="w-8 h-8 flex items-center justify-center rounded-full bg-green-100">
              <svg className="w-5 h-5 text-green-600" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
                <path strokeLinecap="round" strokeLinejoin="round" d="M5 13l4 4L19 7" />
              </svg>
            </div>
            <div>
              <p className="text-sm font-medium text-green-800">{displayName} API Key Configured</p>
              <p className="text-xs text-green-600">Stored securely in your system keychain</p>
            </div>
          </div>
          <button
            type="button"
            onClick={handleDelete}
            className="px-3 py-1.5 text-sm text-red-600 hover:text-red-700 hover:bg-red-50 rounded-lg transition-colors"
          >
            Remove
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className={compact ? '' : 'py-4'}>
      {/* Header */}
      {!compact && (
        <div className="mb-3">
          <h3 className="text-sm font-medium text-stone-700">{displayName} API Key</h3>
          <p className="text-xs text-stone-500 mt-0.5">
            Get your key from{' '}
            <a
              href={keysUrl}
              target="_blank"
              rel="noopener noreferrer"
              className="text-primary-600 hover:text-primary-700 underline"
            >
              {new URL(keysUrl).hostname}
            </a>
          </p>
        </div>
      )}

      {/* Input container */}
      <div
        className={`
          flex items-center gap-3
          px-4 py-3
          bg-white
          border
          rounded-xl
          shadow-sm
          focus-within:ring-2
          transition-all duration-200
          ${getBorderColor()}
        `}
      >
        <div className="text-stone-500">
          <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
            <path strokeLinecap="round" strokeLinejoin="round" d="M15 7a2 2 0 012 2m4 0a6 6 0 01-7.743 5.743L11 17H9v2H7v2H4a1 1 0 01-1-1v-2.586a1 1 0 01.293-.707l5.964-5.964A6 6 0 1121 9z" />
          </svg>
        </div>

        <input
          type="password"
          value={apiKey}
          onChange={(e) => {
            setApiKey(e.target.value);
            setErrorMessage('');
            setStatus('idle');
          }}
          onKeyDown={handleKeyDown}
          placeholder={placeholder}
          disabled={status === 'saving'}
          aria-label={`${displayName} API key`}
          className={`
            flex-1
            bg-transparent
            text-stone-700
            placeholder:text-stone-400
            focus:outline-none
            font-mono text-sm
            ${status === 'saving' ? 'cursor-wait' : ''}
          `}
        />

        {/* Status indicator */}
        {apiKey && (
          <div className="flex-shrink-0">
            {isValid ? (
              <svg className="w-5 h-5 text-green-500" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
                <path strokeLinecap="round" strokeLinejoin="round" d="M5 13l4 4L19 7" />
              </svg>
            ) : (
              <svg className="w-5 h-5 text-stone-300" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
                <path strokeLinecap="round" strokeLinejoin="round" d="M12 8v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
              </svg>
            )}
          </div>
        )}

        <button
          type="button"
          onClick={handleSave}
          disabled={!isValid || status === 'saving'}
          aria-label="Save API key"
          className={`
            px-4 py-2
            flex-shrink-0
            flex items-center gap-2
            rounded-lg
            text-sm font-medium
            transition-all duration-200
            ${
              !isValid || status === 'saving'
                ? 'bg-stone-200 text-stone-500 cursor-not-allowed'
                : 'bg-primary-500 hover:bg-primary-600 text-white shadow-sm hover:shadow-md hover:brightness-110 active:brightness-95'
            }
          `}
        >
          {status === 'saving' ? (
            <>
              <svg className="w-4 h-4 animate-spin-slow" fill="none" viewBox="0 0 24 24">
                <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
                <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z" />
              </svg>
              Saving
            </>
          ) : status === 'saved' ? (
            <>
              <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
                <path strokeLinecap="round" strokeLinejoin="round" d="M5 13l4 4L19 7" />
              </svg>
              Saved
            </>
          ) : (
            'Save Key'
          )}
        </button>
      </div>

      {/* Error message */}
      {errorMessage && (
        <p className="mt-2 text-sm text-red-600" role="alert" aria-live="assertive">{errorMessage}</p>
      )}

      {/* Format hint with contextual guidance */}
      {apiKey && !isValid && !errorMessage && (
        <p className="mt-2 text-xs text-amber-600">
          {getApiKeyErrorHint(apiKey, providerId) || (
            <>API key should start with <code className="bg-stone-100 px-1 rounded">{placeholder}</code></>
          )}
        </p>
      )}
    </div>
  );
}

export default ApiKeyInput;
