import { useState, useCallback, useEffect } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import {
  getDocumentFolder,
  setDocumentFolder,
  removeDocumentFolder,
  rescanDocuments,
} from '../../lib/tauri-commands';
import type { DocumentFolderStats } from '../../lib/types';

interface DocumentFolderConfigProps {
  compact?: boolean;
}

export function DocumentFolderConfig({ compact: _compact = false }: DocumentFolderConfigProps) {
  const [stats, setStats] = useState<DocumentFolderStats | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [isScanning, setIsScanning] = useState(false);
  const [error, setError] = useState('');

  useEffect(() => {
    setIsLoading(true);
    getDocumentFolder()
      .then(setStats)
      .catch(() => setStats(null))
      .finally(() => setIsLoading(false));
  }, []);

  const handleChooseFolder = useCallback(async () => {
    try {
      const selected = await open({ directory: true, multiple: false });
      if (!selected) return; // User cancelled

      setIsScanning(true);
      setError('');
      const result = await setDocumentFolder(selected as string);
      setStats(result);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsScanning(false);
    }
  }, []);

  const handleRescan = useCallback(async () => {
    setIsScanning(true);
    setError('');
    try {
      const result = await rescanDocuments();
      setStats(result);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsScanning(false);
    }
  }, []);

  const handleRemove = useCallback(async () => {
    try {
      await removeDocumentFolder();
      setStats(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }, []);

  if (isLoading) {
    return <div className="p-4 text-sm text-stone-500">Loading...</div>;
  }

  // Scanning state
  if (isScanning) {
    return (
      <div className="p-4 bg-stone-50 border border-stone-200 rounded-xl">
        <div className="flex items-center gap-3">
          <svg className="w-5 h-5 text-primary-500 animate-spin" fill="none" viewBox="0 0 24 24">
            <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
            <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z" />
          </svg>
          <p className="text-sm text-stone-600">Indexing documents...</p>
        </div>
      </div>
    );
  }

  // No folder configured — empty state
  if (!stats) {
    return (
      <div className="p-4 bg-stone-50 border border-stone-200 rounded-xl">
        <div className="text-center space-y-3">
          <div className="w-10 h-10 mx-auto rounded-full bg-stone-200 flex items-center justify-center">
            <svg className="w-5 h-5 text-stone-500" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
              <path strokeLinecap="round" strokeLinejoin="round" d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
            </svg>
          </div>
          <div>
            <p className="text-sm text-stone-700">
              Point Alex at a folder of HR documents — policies, handbooks, meeting notes —
              and Alex will use them to answer your questions.
            </p>
            <p className="text-xs text-stone-500 mt-1">
              Files stay on your machine. Sensitive data is automatically redacted.
            </p>
          </div>
          <button
            type="button"
            onClick={handleChooseFolder}
            className="px-4 py-2 text-sm font-medium text-white bg-primary-500 hover:bg-primary-600 rounded-lg transition-colors"
          >
            Choose Folder
          </button>
        </div>
        {error && <p className="mt-2 text-sm text-red-600">{error}</p>}
      </div>
    );
  }

  // Folder configured — show stats
  const folderName = stats.path.split('/').pop() || stats.path;

  // Format last scanned time
  const lastScan = stats.last_scanned_at
    ? new Date(stats.last_scanned_at + 'Z').toLocaleString()
    : 'Never';

  return (
    <div className="p-4 bg-stone-50 border border-stone-200 rounded-xl space-y-3">
      <div className="flex items-center justify-between gap-3">
        <div className="flex items-center gap-3 min-w-0">
          <div className="w-8 h-8 flex-shrink-0 flex items-center justify-center rounded-full bg-primary-100">
            <svg className="w-4 h-4 text-primary-600" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
              <path strokeLinecap="round" strokeLinejoin="round" d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
            </svg>
          </div>
          <div className="min-w-0">
            <p className="text-sm font-medium text-stone-700 truncate" title={stats.path}>
              {folderName}
            </p>
            <p className="text-xs text-stone-500">
              {stats.file_count} files indexed
              {stats.chunk_count > 0 && ` \u00B7 ${stats.chunk_count} sections`}
              {stats.last_scanned_at && ` \u00B7 ${lastScan}`}
            </p>
          </div>
        </div>
        <button
          type="button"
          onClick={handleChooseFolder}
          className="flex-shrink-0 text-sm text-primary-600 hover:text-primary-700"
        >
          Change
        </button>
      </div>

      {/* PII warning */}
      {stats.pii_file_count > 0 && (
        <div className="flex items-center gap-2 px-3 py-2 bg-amber-50 border border-amber-200 rounded-lg">
          <svg className="w-4 h-4 text-amber-600 flex-shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
            <path strokeLinecap="round" strokeLinejoin="round" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
          </svg>
          <p className="text-xs text-amber-800">
            {stats.pii_file_count} file{stats.pii_file_count > 1 ? 's' : ''} contained sensitive data (auto-redacted)
          </p>
        </div>
      )}

      {/* Error warning */}
      {stats.error_file_count > 0 && (
        <div className="flex items-center gap-2 px-3 py-2 bg-red-50 border border-red-200 rounded-lg">
          <svg className="w-4 h-4 text-red-600 flex-shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
            <path strokeLinecap="round" strokeLinejoin="round" d="M12 8v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
          </svg>
          <p className="text-xs text-red-800">
            {stats.error_file_count} file{stats.error_file_count > 1 ? 's' : ''} could not be parsed
          </p>
        </div>
      )}

      {/* Actions */}
      <div className="flex gap-2">
        <button
          type="button"
          onClick={handleRescan}
          className="px-3 py-1.5 text-sm text-stone-600 hover:text-stone-800 hover:bg-stone-100 rounded-lg transition-colors"
        >
          Re-scan Now
        </button>
        <button
          type="button"
          onClick={handleRemove}
          className="px-3 py-1.5 text-sm text-red-600 hover:text-red-700 hover:bg-red-50 rounded-lg transition-colors"
        >
          Remove
        </button>
      </div>

      {error && <p className="text-sm text-red-600">{error}</p>}
    </div>
  );
}

export default DocumentFolderConfig;
