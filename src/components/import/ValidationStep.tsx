/**
 * ValidationStep Component (V2.5.1d)
 *
 * Displays validation results after column mapping.
 * Shows summary bar + scrollable issue table.
 * Provides "Fix Issues" or "Continue" actions.
 */

import { useEffect, useRef } from 'react';
import type { ImportValidationResult } from '../../lib/types';

// =============================================================================
// Types
// =============================================================================

interface ValidationStepProps {
  validationResult: ImportValidationResult;
  totalRows: number;
  onFixIssues: () => void;
  onContinue: () => void;
  onBack: () => void;
}

// =============================================================================
// Main Component
// =============================================================================

export function ValidationStep({
  validationResult,
  totalRows,
  onFixIssues,
  onContinue,
  onBack,
}: ValidationStepProps) {
  const summaryRef = useRef<HTMLDivElement>(null);

  // Focus summary on mount for screen readers
  useEffect(() => {
    summaryRef.current?.focus();
  }, []);

  const { issues, errorRowCount, warningRowCount, cleanRowCount } = validationResult;
  const hasErrors = errorRowCount > 0;
  const hasWarnings = warningRowCount > 0;

  return (
    <div className="bg-white rounded-xl border border-stone-200 shadow-sm overflow-hidden">
      {/* Summary Bar */}
      <div
        ref={summaryRef}
        tabIndex={-1}
        className="px-6 py-4 border-b border-stone-200 bg-stone-50 outline-none"
        role="status"
        aria-live="polite"
      >
        <h3 className="text-lg font-medium text-stone-900">Validation Results</h3>
        <p className="mt-1 text-sm text-stone-600">
          {validationResult.isValid
            ? `All ${totalRows} rows passed validation.`
            : `Found issues in ${totalRows} rows.`}
        </p>

        <ValidationSummaryBar
          errorCount={errorRowCount}
          warningCount={warningRowCount}
          cleanCount={cleanRowCount}
          totalRows={totalRows}
        />
      </div>

      {/* Issue Table */}
      {issues.length > 0 && (
        <div className="max-h-[40vh] overflow-y-auto">
          <ValidationIssueTable issues={issues} />
        </div>
      )}

      {/* Actions */}
      <div className="px-6 py-4 border-t border-stone-200 bg-white flex items-center justify-between">
        <button
          type="button"
          onClick={onBack}
          className="px-4 py-2 text-sm font-medium text-stone-700 bg-white border border-stone-300 rounded-lg hover:bg-stone-50 transition-colors"
        >
          Back to Mapping
        </button>

        <div className="flex items-center gap-3">
          {hasErrors && (
            <button
              type="button"
              onClick={onFixIssues}
              className="px-4 py-2 text-sm font-medium text-amber-700 bg-amber-50 border border-amber-200 rounded-lg hover:bg-amber-100 transition-colors"
            >
              Fix Issues
            </button>
          )}

          <button
            type="button"
            onClick={onContinue}
            className={`
              px-4 py-2 text-sm font-medium text-white rounded-lg transition-all
              ${
                hasErrors
                  ? 'bg-stone-400 hover:bg-stone-500'
                  : 'bg-primary-500 hover:bg-primary-600'
              }
            `}
          >
            {hasErrors
              ? `Skip ${errorRowCount} Error Row${errorRowCount !== 1 ? 's' : ''}`
              : hasWarnings
                ? 'Continue with Warnings'
                : 'Continue'}
          </button>
        </div>
      </div>
    </div>
  );
}

// =============================================================================
// Sub-components
// =============================================================================

function ValidationSummaryBar({
  errorCount,
  warningCount,
  cleanCount,
  totalRows,
}: {
  errorCount: number;
  warningCount: number;
  cleanCount: number;
  totalRows: number;
}) {
  if (totalRows === 0) return null;

  const errorPct = (errorCount / totalRows) * 100;
  const warningPct = (warningCount / totalRows) * 100;
  const cleanPct = (cleanCount / totalRows) * 100;

  return (
    <div className="mt-3">
      {/* Bar */}
      <div
        className="h-2 rounded-full overflow-hidden flex bg-stone-200"
        role="progressbar"
        aria-label={`${cleanCount} clean, ${warningCount} warnings, ${errorCount} errors`}
      >
        {cleanPct > 0 && (
          <div
            className="bg-primary-400 transition-all"
            style={{ width: `${cleanPct}%` }}
          />
        )}
        {warningPct > 0 && (
          <div
            className="bg-amber-400 transition-all"
            style={{ width: `${warningPct}%` }}
          />
        )}
        {errorPct > 0 && (
          <div
            className="bg-red-400 transition-all"
            style={{ width: `${errorPct}%` }}
          />
        )}
      </div>

      {/* Legend */}
      <div className="mt-2 flex items-center gap-4 text-xs text-stone-600">
        <span className="flex items-center gap-1.5">
          <span className="w-2 h-2 rounded-full bg-primary-400" aria-hidden="true" />
          {cleanCount} clean
        </span>
        {warningCount > 0 && (
          <span className="flex items-center gap-1.5">
            <span className="w-2 h-2 rounded-full bg-amber-400" aria-hidden="true" />
            {warningCount} warning{warningCount !== 1 ? 's' : ''}
          </span>
        )}
        {errorCount > 0 && (
          <span className="flex items-center gap-1.5">
            <span className="w-2 h-2 rounded-full bg-red-400" aria-hidden="true" />
            {errorCount} error{errorCount !== 1 ? 's' : ''}
          </span>
        )}
      </div>
    </div>
  );
}

function ValidationIssueTable({
  issues,
}: {
  issues: ImportValidationResult['issues'];
}) {
  return (
    <table className="w-full" role="table" aria-label="Validation Issues">
      <thead className="sticky top-0">
        <tr className="bg-stone-50 border-b border-stone-200">
          <th className="px-4 py-2 text-left text-xs font-semibold text-stone-500 uppercase tracking-wider">
            Row
          </th>
          <th className="px-4 py-2 text-left text-xs font-semibold text-stone-500 uppercase tracking-wider">
            Column
          </th>
          <th className="px-4 py-2 text-left text-xs font-semibold text-stone-500 uppercase tracking-wider">
            Value
          </th>
          <th className="px-4 py-2 text-left text-xs font-semibold text-stone-500 uppercase tracking-wider">
            Issue
          </th>
          <th className="px-4 py-2 text-left text-xs font-semibold text-stone-500 uppercase tracking-wider">
            Severity
          </th>
        </tr>
      </thead>
      <tbody className="divide-y divide-stone-100">
        {issues.map((issue, idx) => (
          <tr
            key={idx}
            className={idx % 2 === 0 ? 'bg-white' : 'bg-stone-50/50'}
          >
            <td className="px-4 py-2 text-sm text-stone-700 font-mono">
              {issue.row}
            </td>
            <td className="px-4 py-2 text-sm text-stone-700">
              {issue.column}
            </td>
            <td className="px-4 py-2 text-sm text-stone-500 font-mono max-w-[150px] truncate">
              {issue.value || <span className="italic text-stone-400">(empty)</span>}
            </td>
            <td className="px-4 py-2 text-sm text-stone-700">
              {issue.message}
            </td>
            <td className="px-4 py-2">
              <SeverityBadge severity={issue.severity} />
            </td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

function SeverityBadge({ severity }: { severity: 'error' | 'warning' }) {
  if (severity === 'error') {
    return (
      <span
        className="inline-flex items-center gap-1 px-2 py-0.5 rounded text-xs font-medium bg-red-100 text-red-700"
        role="status"
      >
        <svg className="w-3 h-3" fill="currentColor" viewBox="0 0 20 20" aria-hidden="true">
          <path
            fillRule="evenodd"
            d="M10 18a8 8 0 100-16 8 8 0 000 16zM8.28 7.22a.75.75 0 00-1.06 1.06L8.94 10l-1.72 1.72a.75.75 0 101.06 1.06L10 11.06l1.72 1.72a.75.75 0 101.06-1.06L11.06 10l1.72-1.72a.75.75 0 00-1.06-1.06L10 8.94 8.28 7.22z"
            clipRule="evenodd"
          />
        </svg>
        Error
      </span>
    );
  }

  return (
    <span
      className="inline-flex items-center gap-1 px-2 py-0.5 rounded text-xs font-medium bg-amber-100 text-amber-700"
      role="status"
    >
      <svg className="w-3 h-3" fill="currentColor" viewBox="0 0 20 20" aria-hidden="true">
        <path
          fillRule="evenodd"
          d="M8.485 2.495c.673-1.167 2.357-1.167 3.03 0l6.28 10.875c.673 1.167-.17 2.625-1.516 2.625H3.72c-1.347 0-2.189-1.458-1.515-2.625L8.485 2.495zM10 5a.75.75 0 01.75.75v3.5a.75.75 0 01-1.5 0v-3.5A.75.75 0 0110 5zm0 9a1 1 0 100-2 1 1 0 000 2z"
          clipRule="evenodd"
        />
      </svg>
      Warning
    </span>
  );
}

export default ValidationStep;
