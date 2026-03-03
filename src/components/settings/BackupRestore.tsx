/**
 * BackupRestore Component
 *
 * Provides UI for exporting encrypted backups and restoring from backup files.
 * Uses AES-256-GCM encryption with Argon2 key derivation.
 */

import { useState, useRef, useCallback } from 'react';
import {
  exportBackup,
  validateBackup,
  importBackup,
  downloadBackupFile,
  readBackupFileAsBytes,
  type BackupMetadata,
  type BackupTableCounts,
} from '../../lib/tauri-commands';

interface BackupRestoreProps {
  /** Called after a successful import to trigger app refresh */
  onImportComplete?: () => void;
}

type Status = 'idle' | 'exporting' | 'validating' | 'previewing' | 'importing';

export function BackupRestore({ onImportComplete }: BackupRestoreProps) {
  // Export state
  const [exportPassword, setExportPassword] = useState('');
  const [confirmPassword, setConfirmPassword] = useState('');
  const [showExportForm, setShowExportForm] = useState(false);

  // Import state
  const [importPassword, setImportPassword] = useState('');
  const [selectedFile, setSelectedFile] = useState<File | null>(null);
  const [previewMetadata, setPreviewMetadata] = useState<BackupMetadata | null>(null);
  const [showImportForm, setShowImportForm] = useState(false);

  // Shared state
  const [status, setStatus] = useState<Status>('idle');
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);

  const fileInputRef = useRef<HTMLInputElement>(null);

  const resetExportForm = useCallback(() => {
    setExportPassword('');
    setConfirmPassword('');
    setShowExportForm(false);
    setError(null);
  }, []);

  const resetImportForm = useCallback(() => {
    setImportPassword('');
    setSelectedFile(null);
    setPreviewMetadata(null);
    setShowImportForm(false);
    setError(null);
    if (fileInputRef.current) {
      fileInputRef.current.value = '';
    }
  }, []);

  const handleExport = useCallback(async () => {
    if (exportPassword.length < 8) {
      setError('Password must be at least 8 characters');
      return;
    }
    if (exportPassword !== confirmPassword) {
      setError('Passwords do not match');
      return;
    }

    setStatus('exporting');
    setError(null);

    try {
      const result = await exportBackup(exportPassword);
      downloadBackupFile(result.encrypted_data, result.filename);

      const totalRecords = Object.values(result.table_counts).reduce((a, b) => a + b, 0);
      setSuccess(`Backup created: ${totalRecords} records exported`);
      resetExportForm();

      // Clear success message after 5 seconds
      setTimeout(() => setSuccess(null), 5000);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Export failed');
    } finally {
      setStatus('idle');
    }
  }, [exportPassword, confirmPassword, resetExportForm]);

  const handleFileSelect = useCallback(async (file: File) => {
    setSelectedFile(file);
    setPreviewMetadata(null);
    setError(null);
  }, []);

  const handleValidate = useCallback(async () => {
    if (!selectedFile) return;
    if (importPassword.length < 8) {
      setError('Password must be at least 8 characters');
      return;
    }

    setStatus('validating');
    setError(null);

    try {
      const data = await readBackupFileAsBytes(selectedFile);
      const metadata = await validateBackup(data, importPassword);
      setPreviewMetadata(metadata);
      setStatus('previewing');
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Validation failed';
      if (message.includes('password') || message.includes('InvalidPassword')) {
        setError('Incorrect password. Please try again.');
      } else if (message.includes('invalid') || message.includes('InvalidBackup')) {
        setError('This backup file appears to be damaged or is not a valid People Partner backup.');
      } else {
        setError(message);
      }
      setStatus('idle');
    }
  }, [selectedFile, importPassword]);

  const handleImport = useCallback(async () => {
    if (!selectedFile || !previewMetadata) return;

    setStatus('importing');
    setError(null);

    try {
      const data = await readBackupFileAsBytes(selectedFile);
      const result = await importBackup(data, importPassword);

      const totalRecords = Object.values(result.restored_counts).reduce((a, b) => a + b, 0);
      setSuccess(`Backup restored: ${totalRecords} records imported`);
      resetImportForm();

      // Trigger app refresh after short delay
      setTimeout(() => {
        onImportComplete?.();
      }, 1500);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Import failed');
      setStatus('previewing');
    }
  }, [selectedFile, previewMetadata, importPassword, resetImportForm, onImportComplete]);

  const formatTableCounts = (counts: BackupTableCounts): string => {
    const parts: string[] = [];
    if (counts.employees > 0) parts.push(`${counts.employees} employees`);
    if (counts.conversations > 0) parts.push(`${counts.conversations} conversations`);
    if (counts.performance_ratings > 0) parts.push(`${counts.performance_ratings} ratings`);
    if (counts.performance_reviews > 0) parts.push(`${counts.performance_reviews} reviews`);
    if (counts.enps_responses > 0) parts.push(`${counts.enps_responses} eNPS`);
    return parts.join(', ') || 'No data';
  };

  return (
    <div className="space-y-4">
      {/* Success Message */}
      {success && (
        <div className="flex items-center gap-2 p-3 bg-green-50 border border-green-200 rounded-lg text-sm text-green-700">
          <svg className="w-4 h-4 flex-shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
            <path strokeLinecap="round" strokeLinejoin="round" d="M5 13l4 4L19 7" />
          </svg>
          {success}
        </div>
      )}

      {/* Error Message */}
      {error && (
        <div className="flex items-center gap-2 p-3 bg-red-50 border border-red-200 rounded-lg text-sm text-red-700">
          <svg className="w-4 h-4 flex-shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
            <path strokeLinecap="round" strokeLinejoin="round" d="M12 8v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
          </svg>
          {error}
        </div>
      )}

      {/* Export Section */}
      <div className="p-4 bg-stone-50 border border-stone-200 rounded-xl">
        <div className="flex items-center justify-between gap-4">
          <div className="flex items-center gap-3">
            <div className="w-8 h-8 flex-shrink-0 flex items-center justify-center rounded-full bg-stone-200">
              <svg className="w-4 h-4 text-stone-600" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
                <path strokeLinecap="round" strokeLinejoin="round" d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4" />
              </svg>
            </div>
            <div>
              <p className="text-sm font-medium text-stone-700">Export Backup</p>
              <p className="text-xs text-stone-500">Download encrypted backup of all data</p>
            </div>
          </div>
          {!showExportForm && (
            <button
              type="button"
              onClick={() => setShowExportForm(true)}
              disabled={status !== 'idle'}
              className="px-4 py-2 text-sm font-medium text-white bg-primary-500 hover:bg-primary-600 rounded-lg transition-colors disabled:opacity-50"
            >
              Export
            </button>
          )}
        </div>

        {showExportForm && (
          <div className="mt-4 pt-4 border-t border-stone-200 space-y-3">
            <div>
              <label className="block text-xs font-medium text-stone-600 mb-1">
                Encryption Password
              </label>
              <input
                type="password"
                value={exportPassword}
                onChange={(e) => setExportPassword(e.target.value)}
                placeholder="Minimum 8 characters"
                className="w-full px-3 py-2 text-sm border border-stone-300 rounded-lg focus:outline-none focus:ring-2 focus:ring-primary-500 focus:border-transparent"
              />
            </div>
            <div>
              <label className="block text-xs font-medium text-stone-600 mb-1">
                Confirm Password
              </label>
              <input
                type="password"
                value={confirmPassword}
                onChange={(e) => setConfirmPassword(e.target.value)}
                placeholder="Enter password again"
                className="w-full px-3 py-2 text-sm border border-stone-300 rounded-lg focus:outline-none focus:ring-2 focus:ring-primary-500 focus:border-transparent"
              />
            </div>
            <div className="flex justify-end gap-2">
              <button
                type="button"
                onClick={resetExportForm}
                className="px-3 py-1.5 text-sm text-stone-600 hover:text-stone-800"
              >
                Cancel
              </button>
              <button
                type="button"
                onClick={handleExport}
                disabled={status === 'exporting' || exportPassword.length < 8}
                className="px-4 py-1.5 text-sm font-medium text-white bg-primary-500 hover:bg-primary-600 rounded-lg transition-colors disabled:opacity-50"
              >
                {status === 'exporting' ? 'Exporting...' : 'Create Backup'}
              </button>
            </div>
          </div>
        )}
      </div>

      {/* Import Section */}
      <div className="p-4 bg-stone-50 border border-stone-200 rounded-xl">
        <div className="flex items-center justify-between gap-4">
          <div className="flex items-center gap-3">
            <div className="w-8 h-8 flex-shrink-0 flex items-center justify-center rounded-full bg-stone-200">
              <svg className="w-4 h-4 text-stone-600" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
                <path strokeLinecap="round" strokeLinejoin="round" d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-8l-4-4m0 0L8 8m4-4v12" />
              </svg>
            </div>
            <div>
              <p className="text-sm font-medium text-stone-700">Restore from Backup</p>
              <p className="text-xs text-stone-500">Replace all data from a backup file</p>
            </div>
          </div>
          {!showImportForm && (
            <button
              type="button"
              onClick={() => setShowImportForm(true)}
              disabled={status !== 'idle'}
              className="px-4 py-2 text-sm font-medium text-stone-700 bg-white border border-stone-300 hover:bg-stone-50 rounded-lg transition-colors disabled:opacity-50"
            >
              Restore
            </button>
          )}
        </div>

        {showImportForm && (
          <div className="mt-4 pt-4 border-t border-stone-200 space-y-3">
            {/* File Selection */}
            <div>
              <label className="block text-xs font-medium text-stone-600 mb-1">
                Backup File
              </label>
              <input
                ref={fileInputRef}
                type="file"
                accept=".hrbackup"
                onChange={(e) => {
                  const file = e.target.files?.[0];
                  if (file) handleFileSelect(file);
                }}
                className="w-full text-sm text-stone-600 file:mr-3 file:py-1.5 file:px-3 file:rounded-lg file:border-0 file:text-sm file:font-medium file:bg-stone-200 file:text-stone-700 hover:file:bg-stone-300"
              />
            </div>

            {/* Password Input */}
            {selectedFile && !previewMetadata && (
              <div>
                <label className="block text-xs font-medium text-stone-600 mb-1">
                  Backup Password
                </label>
                <input
                  type="password"
                  value={importPassword}
                  onChange={(e) => setImportPassword(e.target.value)}
                  placeholder="Enter the backup password"
                  className="w-full px-3 py-2 text-sm border border-stone-300 rounded-lg focus:outline-none focus:ring-2 focus:ring-primary-500 focus:border-transparent"
                />
              </div>
            )}

            {/* Preview Metadata */}
            {previewMetadata && (
              <div className="p-3 bg-amber-50 border border-amber-200 rounded-lg">
                <div className="flex items-start gap-2">
                  <svg className="w-4 h-4 text-amber-600 mt-0.5 flex-shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
                    <path strokeLinecap="round" strokeLinejoin="round" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
                  </svg>
                  <div className="text-sm">
                    <p className="font-medium text-amber-800">This will replace all existing data!</p>
                    <p className="text-amber-700 mt-1">
                      Backup from: {new Date(previewMetadata.created_at).toLocaleString()}
                    </p>
                    <p className="text-amber-700">
                      Contains: {formatTableCounts(previewMetadata.table_counts)}
                    </p>
                  </div>
                </div>
              </div>
            )}

            {/* Action Buttons */}
            <div className="flex justify-end gap-2">
              <button
                type="button"
                onClick={resetImportForm}
                className="px-3 py-1.5 text-sm text-stone-600 hover:text-stone-800"
              >
                Cancel
              </button>
              {!previewMetadata ? (
                <button
                  type="button"
                  onClick={handleValidate}
                  disabled={status === 'validating' || !selectedFile || importPassword.length < 8}
                  className="px-4 py-1.5 text-sm font-medium text-white bg-primary-500 hover:bg-primary-600 rounded-lg transition-colors disabled:opacity-50"
                >
                  {status === 'validating' ? 'Validating...' : 'Validate'}
                </button>
              ) : (
                <button
                  type="button"
                  onClick={handleImport}
                  disabled={status === 'importing'}
                  className="px-4 py-1.5 text-sm font-medium text-white bg-red-500 hover:bg-red-600 rounded-lg transition-colors disabled:opacity-50"
                >
                  {status === 'importing' ? 'Restoring...' : 'Restore Backup'}
                </button>
              )}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

export default BackupRestore;
