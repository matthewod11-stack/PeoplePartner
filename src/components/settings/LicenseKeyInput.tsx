import { useState, useCallback, useEffect } from 'react';
import {
  storeLicenseKey,
  hasLicenseKey,
  deleteLicenseKey,
  validateLicenseKeyFormat,
} from '../../lib/tauri-commands';

type LicenseStatus = 'idle' | 'saving' | 'saved' | 'error';

interface LicenseKeyInputProps {
  onSave?: () => void;
  onDelete?: () => void;
  compact?: boolean;
}

export function LicenseKeyInput({
  onSave,
  onDelete,
  compact = false,
}: LicenseKeyInputProps) {
  const [licenseKey, setLicenseKey] = useState('');
  const [status, setStatus] = useState<LicenseStatus>('idle');
  const [hasExistingKey, setHasExistingKey] = useState(false);
  const [isValid, setIsValid] = useState(false);
  const [errorMessage, setErrorMessage] = useState('');

  useEffect(() => {
    hasLicenseKey()
      .then(setHasExistingKey)
      .catch(() => setHasExistingKey(false));
  }, []);

  useEffect(() => {
    if (!licenseKey) {
      setIsValid(false);
      return;
    }

    const timer = setTimeout(async () => {
      try {
        const valid = await validateLicenseKeyFormat(licenseKey);
        setIsValid(valid);
      } catch {
        setIsValid(false);
      }
    }, 250);

    return () => clearTimeout(timer);
  }, [licenseKey]);

  const handleSave = useCallback(async () => {
    if (!licenseKey || !isValid) return;

    setStatus('saving');
    setErrorMessage('');
    try {
      await storeLicenseKey(licenseKey);
      setStatus('saved');
      setHasExistingKey(true);
      setLicenseKey('');
      onSave?.();
      setTimeout(() => setStatus('idle'), 2000);
    } catch (err) {
      setStatus('error');
      setErrorMessage(err instanceof Error ? err.message : 'Failed to save license key');
    }
  }, [isValid, licenseKey, onSave]);

  const handleDelete = useCallback(async () => {
    try {
      await deleteLicenseKey();
      setHasExistingKey(false);
      setStatus('idle');
      onDelete?.();
    } catch (err) {
      setErrorMessage(err instanceof Error ? err.message : 'Failed to remove license key');
    }
  }, [onDelete]);

  if (hasExistingKey && status !== 'saved') {
    return (
      <div className={compact ? '' : 'py-4'}>
        <div className="flex items-center justify-between gap-4 p-4 bg-blue-50 border border-blue-200 rounded-xl">
          <div className="flex items-center gap-3">
            <div className="w-8 h-8 flex items-center justify-center rounded-full bg-blue-100">
              <svg className="w-5 h-5 text-blue-600" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
                <path strokeLinecap="round" strokeLinejoin="round" d="M5 13l4 4L19 7" />
              </svg>
            </div>
            <div>
              <p className="text-sm font-medium text-blue-800">License Active</p>
              <p className="text-xs text-blue-600">Paid mode unlocked on this device</p>
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
      {!compact && (
        <div className="mb-3">
          <h3 className="text-sm font-medium text-stone-700">Purchase License Key</h3>
          <p className="text-xs text-stone-500 mt-0.5">
            Enter the key from your purchase email to unlock paid mode.
          </p>
        </div>
      )}

      <div
        className={`
          flex items-center gap-3
          px-4 py-3
          bg-white border rounded-xl shadow-sm focus-within:ring-2 transition-all duration-200
          ${
            errorMessage
              ? 'border-red-300 focus-within:border-red-400 focus-within:ring-red-100'
              : isValid && licenseKey
                ? 'border-green-300 focus-within:border-green-400 focus-within:ring-green-100'
                : 'border-stone-200 focus-within:border-primary-300 focus-within:ring-primary-100'
          }
        `}
      >
        <div className="text-stone-500">
          <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
            <path strokeLinecap="round" strokeLinejoin="round" d="M12 15v2m3 4H9a2 2 0 01-2-2v-3H5a2 2 0 01-2-2V9a2 2 0 012-2h2V5a2 2 0 012-2h6a2 2 0 012 2v2h2a2 2 0 012 2v5a2 2 0 01-2 2h-2v3a2 2 0 01-2 2z" />
          </svg>
        </div>

        <input
          type="text"
          value={licenseKey}
          onChange={(e) => {
            setLicenseKey(e.target.value.toUpperCase());
            setErrorMessage('');
            setStatus('idle');
          }}
          placeholder="HRC-XXXX-XXXX-XXXX"
          disabled={status === 'saving'}
          aria-label="License key"
          className={`
            flex-1 bg-transparent text-stone-700 placeholder:text-stone-400 focus:outline-none font-mono text-sm
            ${status === 'saving' ? 'cursor-wait' : ''}
          `}
        />

        {licenseKey && (
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
          className={`
            px-4 py-2 rounded-lg text-sm font-medium transition-all duration-200
            ${
              !isValid || status === 'saving'
                ? 'bg-stone-200 text-stone-500 cursor-not-allowed'
                : 'bg-primary-500 hover:bg-primary-600 text-white shadow-sm hover:shadow-md hover:brightness-110 active:brightness-95'
            }
          `}
        >
          {status === 'saving' ? 'Saving' : status === 'saved' ? 'Saved' : 'Save Key'}
        </button>
      </div>

      {errorMessage && (
        <p className="mt-2 text-sm text-red-600" role="alert" aria-live="assertive">
          {errorMessage}
        </p>
      )}

      {licenseKey && !isValid && !errorMessage && (
        <p className="mt-2 text-xs text-amber-600">
          Use letters, numbers, and dashes (for example: <code className="bg-stone-100 px-1 rounded">HRC-XXXX-XXXX</code>).
        </p>
      )}
    </div>
  );
}

export default LicenseKeyInput;
