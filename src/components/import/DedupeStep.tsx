/**
 * DedupeStep Component (V2.5.1c)
 *
 * Shows potential duplicate records found between import file and existing data.
 * Users can choose to keep new, keep existing, or skip each duplicate.
 */

import { useState, useCallback, useEffect, useRef } from 'react';
import type { DuplicateGroup, DuplicateResolution } from '../../lib/types';

// =============================================================================
// Types
// =============================================================================

interface DedupeStepProps {
  duplicates: DuplicateGroup[];
  onResolve: (resolutions: DuplicateResolution[]) => void;
  onBack: () => void;
}

// =============================================================================
// Main Component
// =============================================================================

export function DedupeStep({ duplicates, onResolve, onBack }: DedupeStepProps) {
  const headingRef = useRef<HTMLHeadingElement>(null);
  const [resolutions, setResolutions] = useState<Map<string, DuplicateResolution['action']>>(
    () => new Map(duplicates.map((d) => [d.id, 'keep_new']))
  );

  // Focus heading on mount
  useEffect(() => {
    headingRef.current?.focus();
  }, []);

  const allResolved = resolutions.size === duplicates.length;

  const handleResolutionChange = useCallback(
    (groupId: string, action: DuplicateResolution['action']) => {
      setResolutions((prev) => {
        const next = new Map(prev);
        next.set(groupId, action);
        return next;
      });
    },
    []
  );

  const handleBulkAction = useCallback(
    (action: DuplicateResolution['action']) => {
      setResolutions(
        new Map(duplicates.map((d) => [d.id, action]))
      );
    },
    [duplicates]
  );

  const handleConfirm = useCallback(() => {
    const resolved: DuplicateResolution[] = Array.from(
      resolutions.entries()
    ).map(([groupId, action]) => ({ groupId, action }));
    onResolve(resolved);
  }, [resolutions, onResolve]);

  return (
    <div className="bg-white rounded-xl border border-stone-200 shadow-sm overflow-hidden">
      {/* Header */}
      <div className="px-6 py-4 border-b border-stone-200 bg-stone-50">
        <h3
          ref={headingRef}
          tabIndex={-1}
          className="text-lg font-medium text-stone-900 outline-none"
        >
          Duplicate Check
        </h3>
        <p className="mt-1 text-sm text-stone-500" aria-live="polite">
          {duplicates.length} potential duplicate{duplicates.length !== 1 ? 's' : ''}{' '}
          detected. Choose how to handle each one.
        </p>
      </div>

      {/* Bulk Actions */}
      <BulkResolutionBar
        count={duplicates.length}
        onBulkAction={handleBulkAction}
      />

      {/* Duplicate Cards */}
      <div className="max-h-[50vh] overflow-y-auto divide-y divide-stone-200">
        {duplicates.map((group) => (
          <DuplicateCard
            key={group.id}
            group={group}
            resolution={resolutions.get(group.id) ?? 'keep_new'}
            onResolutionChange={(action) =>
              handleResolutionChange(group.id, action)
            }
          />
        ))}
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
        <button
          type="button"
          onClick={handleConfirm}
          disabled={!allResolved}
          className={`
            px-4 py-2 text-sm font-medium text-white rounded-lg transition-all
            ${
              allResolved
                ? 'bg-primary-500 hover:bg-primary-600'
                : 'bg-stone-300 cursor-not-allowed'
            }
          `}
        >
          Continue
        </button>
      </div>
    </div>
  );
}

// =============================================================================
// Sub-components
// =============================================================================

function BulkResolutionBar({
  count,
  onBulkAction,
}: {
  count: number;
  onBulkAction: (action: DuplicateResolution['action']) => void;
}) {
  return (
    <div className="px-6 py-3 border-b border-stone-200 bg-stone-50/50 flex items-center justify-between">
      <span className="text-sm text-stone-600">
        Apply to all {count} duplicate{count !== 1 ? 's' : ''}:
      </span>
      <div className="flex items-center gap-2">
        <button
          type="button"
          onClick={() => onBulkAction('keep_new')}
          className="px-3 py-1 text-xs font-medium text-primary-700 bg-primary-50 border border-primary-200 rounded hover:bg-primary-100 transition-colors"
        >
          Keep All New
        </button>
        <button
          type="button"
          onClick={() => onBulkAction('keep_existing')}
          className="px-3 py-1 text-xs font-medium text-stone-700 bg-stone-100 border border-stone-200 rounded hover:bg-stone-200 transition-colors"
        >
          Keep All Existing
        </button>
        <button
          type="button"
          onClick={() => onBulkAction('skip')}
          className="px-3 py-1 text-xs font-medium text-stone-500 bg-white border border-stone-200 rounded hover:bg-stone-50 transition-colors"
        >
          Skip All
        </button>
      </div>
    </div>
  );
}

function DuplicateCard({
  group,
  resolution,
  onResolutionChange,
}: {
  group: DuplicateGroup;
  resolution: DuplicateResolution['action'];
  onResolutionChange: (action: DuplicateResolution['action']) => void;
}) {
  const radioName = `dupe-${group.id}`;

  // Get comparable fields
  const incomingEmail =
    group.incoming['email'] ||
    group.incoming['Email'] ||
    group.incoming['EMAIL'] ||
    '';
  const incomingName =
    group.incoming['full_name'] ||
    group.incoming['Full Name'] ||
    group.incoming['name'] ||
    '';

  return (
    <div className="px-6 py-4">
      {/* Match info */}
      <div className="flex items-center gap-2 mb-3">
        <span className="text-xs font-medium text-stone-500 bg-stone-100 px-2 py-0.5 rounded">
          {group.matchReason}
        </span>
        <span className="text-xs text-stone-400">
          {Math.round(group.confidence * 100)}% confidence
        </span>
      </div>

      {/* Side-by-side comparison */}
      <div className="grid grid-cols-2 gap-4 mb-3">
        {/* Incoming */}
        <div className="p-3 bg-primary-50/50 border border-primary-200 rounded-lg">
          <div className="text-xs font-semibold text-primary-600 uppercase mb-2">
            New (from file)
          </div>
          <div className="text-sm text-stone-800 font-medium">
            {incomingName || incomingEmail}
          </div>
          {incomingEmail && incomingName && (
            <div className="text-xs text-stone-500 mt-0.5">{incomingEmail}</div>
          )}
        </div>

        {/* Existing */}
        <div className="p-3 bg-stone-50 border border-stone-200 rounded-lg">
          <div className="text-xs font-semibold text-stone-500 uppercase mb-2">
            Existing (in database)
          </div>
          <div className="text-sm text-stone-800 font-medium">
            {group.existing.full_name}
          </div>
          <div className="text-xs text-stone-500 mt-0.5">
            {group.existing.email}
          </div>
          {group.existing.department && (
            <div className="text-xs text-stone-400 mt-0.5">
              {group.existing.department}
            </div>
          )}
        </div>
      </div>

      {/* Resolution Radio Buttons */}
      <fieldset>
        <legend className="sr-only">
          Resolution for {group.existing.email}
        </legend>
        <div className="flex items-center gap-4">
          <label className="flex items-center gap-1.5 cursor-pointer">
            <input
              type="radio"
              name={radioName}
              value="keep_new"
              checked={resolution === 'keep_new'}
              onChange={() => onResolutionChange('keep_new')}
              className="text-primary-500 focus:ring-primary-300"
            />
            <span className="text-sm text-stone-700">Update with new</span>
          </label>
          <label className="flex items-center gap-1.5 cursor-pointer">
            <input
              type="radio"
              name={radioName}
              value="keep_existing"
              checked={resolution === 'keep_existing'}
              onChange={() => onResolutionChange('keep_existing')}
              className="text-stone-500 focus:ring-stone-300"
            />
            <span className="text-sm text-stone-700">Keep existing</span>
          </label>
          <label className="flex items-center gap-1.5 cursor-pointer">
            <input
              type="radio"
              name={radioName}
              value="skip"
              checked={resolution === 'skip'}
              onChange={() => onResolutionChange('skip')}
              className="text-stone-400 focus:ring-stone-300"
            />
            <span className="text-sm text-stone-500">Skip</span>
          </label>
        </div>
      </fieldset>
    </div>
  );
}

export default DedupeStep;
