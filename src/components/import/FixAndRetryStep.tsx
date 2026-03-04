/**
 * FixAndRetryStep Component (V2.5.1e)
 *
 * Shows rows with validation issues in an editable table.
 * Users can fix cell values inline and re-validate.
 */

import { useState, useCallback, useRef, useEffect } from 'react';
import type { ParsedRow, IssueRow, ColumnMapping } from '../../lib/types';
import { useEditableTable } from '../../hooks/useEditableTable';

// =============================================================================
// Types
// =============================================================================

interface FixAndRetryStepProps {
  issueRows: IssueRow[];
  allRows: ParsedRow[];
  columnMapping: ColumnMapping;
  fieldLabels: Record<string, string>;
  onRevalidate: (fixedRows: ParsedRow[]) => void;
  onSkipErrors: () => void;
  onBack: () => void;
}

// =============================================================================
// Main Component
// =============================================================================

export function FixAndRetryStep({
  issueRows,
  allRows,
  columnMapping,
  fieldLabels,
  onRevalidate,
  onSkipErrors,
  onBack,
}: FixAndRetryStepProps) {
  const headingRef = useRef<HTMLHeadingElement>(null);
  const [tableState, tableActions] = useEditableTable({
    initialRows: issueRows.map((ir) => ir.data),
  });

  // Focus heading on mount
  useEffect(() => {
    headingRef.current?.focus();
  }, []);

  // Get mapped fields for columns to show
  const mappedFields = Object.keys(columnMapping).filter(
    (f) => columnMapping[f]
  );

  // Build issue lookup: "rowIdx:columnKey" -> issue message
  const issueMap = new Map<string, string>();
  for (let i = 0; i < issueRows.length; i++) {
    for (const issue of issueRows[i].issues) {
      issueMap.set(`${i}:${issue.column}`, issue.message);
    }
  }

  const handleRevalidate = useCallback(() => {
    // Build full row set: replace issue rows with edited versions
    const editedIssueRows = tableActions.getModifiedRows();
    const issueRowIndices = new Set(issueRows.map((ir) => ir.rowIndex));

    const fixedRows = allRows.map((row, i) => {
      if (issueRowIndices.has(i)) {
        const issueIdx = issueRows.findIndex((ir) => ir.rowIndex === i);
        if (issueIdx >= 0 && issueIdx < editedIssueRows.length) {
          return editedIssueRows[issueIdx];
        }
      }
      return row;
    });

    onRevalidate(fixedRows);
  }, [tableActions, issueRows, allRows, onRevalidate]);

  return (
    <div className="bg-white rounded-xl border border-stone-200 shadow-sm overflow-hidden">
      {/* Header */}
      <div className="px-6 py-4 border-b border-stone-200 bg-stone-50">
        <h3
          ref={headingRef}
          tabIndex={-1}
          className="text-lg font-medium text-stone-900 outline-none"
        >
          Fix Issues
        </h3>
        <p className="mt-1 text-sm text-stone-500">
          {issueRows.length} row{issueRows.length !== 1 ? 's' : ''} with issues.
          Click a cell to edit its value.
        </p>
      </div>

      {/* Editable Table */}
      <div className="max-h-[50vh] overflow-auto">
        <table className="w-full" role="table" aria-label="Fix validation issues">
          <thead className="sticky top-0">
            <tr className="bg-stone-50 border-b border-stone-200">
              <th className="px-3 py-2 text-left text-xs font-semibold text-stone-500 uppercase tracking-wider w-12">
                Row
              </th>
              {mappedFields.map((field) => (
                <th
                  key={field}
                  className="px-3 py-2 text-left text-xs font-semibold text-stone-500 uppercase tracking-wider"
                >
                  {fieldLabels[field] || field}
                </th>
              ))}
            </tr>
          </thead>
          <tbody className="divide-y divide-stone-100">
            {tableState.rows.map((row, rowIdx) => {
              const originalIssueRow = issueRows[rowIdx];

              return (
                <tr key={rowIdx} className="hover:bg-stone-50/50">
                  <td className="px-3 py-2 text-sm text-stone-500 font-mono">
                    {originalIssueRow ? originalIssueRow.rowIndex + 1 : rowIdx + 1}
                  </td>
                  {mappedFields.map((field) => {
                    const header = columnMapping[field];
                    const value = header ? row[header] ?? '' : '';
                    const issueKey = `${rowIdx}:${field}`;
                    const issueMessage = issueMap.get(issueKey);
                    const isEditing =
                      tableState.editingCell?.rowIndex === rowIdx &&
                      tableState.editingCell?.columnKey === (header || field);
                    const isModified = tableState.modifiedCells.has(
                      `${rowIdx}:${header || field}`
                    );

                    return (
                      <td key={field} className="px-3 py-1">
                        <EditableCell
                          value={value}
                          isEditing={isEditing}
                          isModified={isModified}
                          issueMessage={issueMessage}
                          ariaLabel={`Edit ${fieldLabels[field] || field} for row ${
                            originalIssueRow
                              ? originalIssueRow.rowIndex + 1
                              : rowIdx + 1
                          }`}
                          onStartEdit={() =>
                            tableActions.startEdit(rowIdx, header || field)
                          }
                          onCommit={tableActions.commitEdit}
                          onCancel={tableActions.cancelEdit}
                        />
                      </td>
                    );
                  })}
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>

      {/* Footer */}
      <div className="px-6 py-4 border-t border-stone-200 bg-white flex items-center justify-between">
        <button
          type="button"
          onClick={onBack}
          className="px-4 py-2 text-sm font-medium text-stone-700 bg-white border border-stone-300 rounded-lg hover:bg-stone-50 transition-colors"
        >
          Back
        </button>

        <div className="flex items-center gap-3">
          <button
            type="button"
            onClick={onSkipErrors}
            className="px-4 py-2 text-sm font-medium text-stone-600 bg-white border border-stone-300 rounded-lg hover:bg-stone-50 transition-colors"
          >
            Skip Error Rows
          </button>
          <button
            type="button"
            onClick={handleRevalidate}
            disabled={!tableState.hasChanges}
            className={`
              px-4 py-2 text-sm font-medium text-white rounded-lg transition-all
              flex items-center gap-2
              ${
                tableState.hasChanges
                  ? 'bg-primary-500 hover:bg-primary-600'
                  : 'bg-stone-300 cursor-not-allowed'
              }
            `}
          >
            <svg
              className="w-4 h-4"
              fill="none"
              viewBox="0 0 24 24"
              stroke="currentColor"
              strokeWidth={2}
              aria-hidden="true"
            >
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15"
              />
            </svg>
            Re-validate
          </button>
        </div>
      </div>
    </div>
  );
}

// =============================================================================
// EditableCell Sub-component
// =============================================================================

function EditableCell({
  value,
  isEditing,
  isModified,
  issueMessage,
  ariaLabel,
  onStartEdit,
  onCommit,
  onCancel,
}: {
  value: string;
  isEditing: boolean;
  isModified: boolean;
  issueMessage?: string;
  ariaLabel: string;
  onStartEdit: () => void;
  onCommit: (value: string) => void;
  onCancel: () => void;
}) {
  const inputRef = useRef<HTMLInputElement>(null);
  const [editValue, setEditValue] = useState(value);

  // Focus input when editing starts
  useEffect(() => {
    if (isEditing && inputRef.current) {
      inputRef.current.focus();
      inputRef.current.select();
      setEditValue(value);
    }
  }, [isEditing, value]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === 'Enter') {
        e.preventDefault();
        onCommit(editValue);
      } else if (e.key === 'Escape') {
        e.preventDefault();
        onCancel();
      }
    },
    [editValue, onCommit, onCancel]
  );

  if (isEditing) {
    return (
      <input
        ref={inputRef}
        type="text"
        value={editValue}
        onChange={(e) => setEditValue(e.target.value)}
        onKeyDown={handleKeyDown}
        onBlur={() => onCommit(editValue)}
        aria-label={ariaLabel}
        className="w-full px-2 py-1 text-sm border-2 border-primary-400 rounded bg-white focus:outline-none focus:ring-1 focus:ring-primary-300"
      />
    );
  }

  return (
    <button
      type="button"
      onClick={onStartEdit}
      aria-label={ariaLabel}
      aria-describedby={issueMessage ? undefined : undefined}
      className={`
        w-full text-left px-2 py-1 text-sm rounded cursor-pointer
        flex items-center gap-1 group
        ${
          issueMessage
            ? 'bg-red-50 border border-red-200 text-red-800'
            : isModified
              ? 'bg-primary-50 border border-primary-200 text-primary-800'
              : 'border border-transparent hover:border-stone-300 text-stone-700'
        }
      `}
    >
      <span className="flex-1 truncate">
        {value || <span className="italic text-stone-400">(empty)</span>}
      </span>

      {issueMessage && (
        <span title={issueMessage} className="flex-shrink-0">
          <svg
            className="w-3.5 h-3.5 text-red-500"
            fill="currentColor"
            viewBox="0 0 20 20"
            aria-hidden="true"
          >
            <path
              fillRule="evenodd"
              d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-7 4a1 1 0 11-2 0 1 1 0 012 0zm-1-9a1 1 0 00-1 1v4a1 1 0 102 0V6a1 1 0 00-1-1z"
              clipRule="evenodd"
            />
          </svg>
        </span>
      )}

      {isModified && !issueMessage && (
        <svg
          className="w-3 h-3 text-primary-500 flex-shrink-0"
          fill="currentColor"
          viewBox="0 0 20 20"
          aria-hidden="true"
        >
          <path
            fillRule="evenodd"
            d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z"
            clipRule="evenodd"
          />
        </svg>
      )}

      {!issueMessage && !isModified && (
        <svg
          className="w-3 h-3 text-stone-300 opacity-0 group-hover:opacity-100 flex-shrink-0 transition-opacity"
          fill="none"
          viewBox="0 0 24 24"
          stroke="currentColor"
          strokeWidth={2}
          aria-hidden="true"
        >
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            d="M15.232 5.232l3.536 3.536m-2.036-5.036a2.5 2.5 0 113.536 3.536L6.5 21.036H3v-3.572L16.732 3.732z"
          />
        </svg>
      )}
    </button>
  );
}

export default FixAndRetryStep;
