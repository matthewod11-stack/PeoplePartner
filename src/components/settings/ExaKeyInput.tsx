// People Partner — Exa API key input (Recruiting / data-source key, FHR-72).
//
// Intentionally separate from `ApiKeyInput` because Exa is a *data source*,
// not an LLM provider — `ApiKeyInput` is coupled to `PROVIDER_META` (model
// name, console URL, setup steps) which doesn't apply here. This keeps the
// LLM-provider section and the recruiting data-source section conceptually
// distinct in Settings, and leaves room for sibling adapters (Hunter, GitHub,
// X) to follow the same minimal pattern when S3.x lands.
//
// Storage layer is shared: this uses the existing generic
// `storeProviderApiKey('exa', ...)` / `hasProviderApiKey('exa')` /
// `deleteProviderApiKey('exa')` commands, which write under Keychain account
// `exa_api_key` (locked down by a unit test in `src-tauri/src/keyring.rs`).

import { useState, useEffect, useCallback } from 'react';
import {
  storeExaApiKey,
  hasExaApiKey,
  deleteExaApiKey,
} from '../../lib/tauri-commands';

// Exa keys are UUIDs (verified from Exa's docs). A loose UUID regex catches
// fat-finger paste errors without being so strict it rejects valid keys.
const EXA_KEY_PATTERN = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

type Status = 'idle' | 'saving' | 'saved' | 'error';

export function ExaKeyInput() {
  const [apiKey, setApiKey] = useState('');
  const [status, setStatus] = useState<Status>('idle');
  const [hasExisting, setHasExisting] = useState(false);
  const [errorMessage, setErrorMessage] = useState('');

  useEffect(() => {
    hasExaApiKey()
      .then(setHasExisting)
      .catch(() => setHasExisting(false));
  }, []);

  const isValid = EXA_KEY_PATTERN.test(apiKey.trim());

  const handleSave = useCallback(async () => {
    if (!isValid || status === 'saving') return;
    setStatus('saving');
    setErrorMessage('');
    try {
      await storeExaApiKey(apiKey.trim());
      setStatus('saved');
      setHasExisting(true);
      setApiKey('');
      setTimeout(() => setStatus('idle'), 1500);
    } catch (err) {
      setStatus('error');
      setErrorMessage(
        err instanceof Error ? err.message : 'Failed to save Exa key',
      );
    }
  }, [apiKey, isValid, status]);

  const handleDelete = useCallback(async () => {
    try {
      await deleteExaApiKey();
      setHasExisting(false);
      setStatus('idle');
      setErrorMessage('');
    } catch (err) {
      setErrorMessage(
        err instanceof Error ? err.message : 'Failed to remove Exa key',
      );
    }
  }, []);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && isValid && status === 'idle') {
      handleSave();
    }
  };

  if (hasExisting && status !== 'saved') {
    return (
      <div className="flex items-center justify-between gap-4 p-4 bg-green-50 border border-green-200 rounded-xl">
        <div className="flex items-center gap-3">
          <div className="w-8 h-8 flex items-center justify-center rounded-full bg-green-100">
            <svg
              className="w-5 h-5 text-green-600"
              fill="none"
              viewBox="0 0 24 24"
              stroke="currentColor"
              strokeWidth={2}
            >
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                d="M5 13l4 4L19 7"
              />
            </svg>
          </div>
          <div>
            <p className="text-sm font-medium text-green-800">
              Exa API Key Configured
            </p>
            <p className="text-xs text-green-600">
              Stored securely in your system keychain
            </p>
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
    );
  }

  const borderColor = errorMessage
    ? 'border-red-300 focus-within:border-red-400 focus-within:ring-red-100'
    : status === 'saved' || (apiKey && isValid)
      ? 'border-green-300 focus-within:border-green-400 focus-within:ring-green-100'
      : 'border-stone-200 focus-within:border-primary-300 focus-within:ring-primary-100';

  return (
    <div>
      <div className="mb-3">
        <h3 className="text-sm font-medium text-stone-700">Exa API Key</h3>
        <p className="text-xs text-stone-500 mt-0.5">
          Get your key from{' '}
          <a
            href="https://exa.ai"
            target="_blank"
            rel="noopener noreferrer"
            className="text-primary-600 hover:text-primary-700 underline"
          >
            exa.ai
          </a>
          . Used for candidate discovery in the Recruit tab.
        </p>
      </div>

      <div
        className={`
          flex items-center gap-3 px-4 py-3
          bg-white border rounded-xl shadow-sm
          focus-within:ring-2 transition-all duration-200
          ${borderColor}
        `}
      >
        <div className="text-stone-500">
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
              d="M15 7a2 2 0 012 2m4 0a6 6 0 01-7.743 5.743L11 17H9v2H7v2H4a1 1 0 01-1-1v-2.586a1 1 0 01.293-.707l5.964-5.964A6 6 0 1121 9z"
            />
          </svg>
        </div>

        <input
          type="password"
          value={apiKey}
          onChange={(e) => {
            setApiKey(e.target.value);
            setErrorMessage('');
            if (status !== 'saving') setStatus('idle');
          }}
          onKeyDown={handleKeyDown}
          placeholder="00000000-0000-0000-0000-000000000000"
          disabled={status === 'saving'}
          aria-label="Exa API key"
          className="flex-1 bg-transparent text-stone-700 placeholder:text-stone-400 focus:outline-none font-mono text-sm"
        />

        {apiKey && (
          <div className="flex-shrink-0">
            {isValid ? (
              <svg
                className="w-5 h-5 text-green-500"
                fill="none"
                viewBox="0 0 24 24"
                stroke="currentColor"
                strokeWidth={2}
              >
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  d="M5 13l4 4L19 7"
                />
              </svg>
            ) : (
              <svg
                className="w-5 h-5 text-stone-300"
                fill="none"
                viewBox="0 0 24 24"
                stroke="currentColor"
                strokeWidth={2}
              >
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  d="M12 8v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"
                />
              </svg>
            )}
          </div>
        )}

        <button
          type="button"
          onClick={handleSave}
          disabled={!isValid || status === 'saving'}
          aria-label="Save Exa API key"
          className={`
            px-4 py-2 flex-shrink-0 flex items-center gap-2
            rounded-lg text-sm font-medium transition-all duration-200
            ${
              !isValid || status === 'saving'
                ? 'bg-stone-200 text-stone-500 cursor-not-allowed'
                : 'bg-primary-500 hover:bg-primary-600 text-white shadow-sm'
            }
          `}
        >
          {status === 'saving'
            ? 'Saving'
            : status === 'saved'
              ? 'Saved'
              : 'Save Key'}
        </button>
      </div>

      {errorMessage && (
        <p
          className="mt-2 text-sm text-red-600"
          role="alert"
          aria-live="assertive"
        >
          {errorMessage}
        </p>
      )}

      {apiKey && !isValid && !errorMessage && (
        <p className="mt-2 text-xs text-amber-600">
          Exa keys are UUIDs — they look like{' '}
          <code className="bg-stone-100 px-1 rounded">
            xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx
          </code>
          .
        </p>
      )}
    </div>
  );
}

export default ExaKeyInput;
