import { useState, useEffect } from 'react';
import { FileDropzone } from './FileDropzone';
import { ImportPreview } from './ImportPreview';
import { ColumnMappingStep } from './ColumnMappingStep';
import { ValidationStep } from './ValidationStep';
import { FixAndRetryStep } from './FixAndRetryStep';
import { DedupeStep } from './DedupeStep';
import { StepProgress, IMPORT_STEPS, getProgressStepKey } from './StepProgress';
import type { ColumnMapping, ParsedRow, DuplicateResolution, ReviewCycle } from '../../lib/types';
import type { CreateRatingInput } from '../../lib/tauri-commands';
import {
  mapRatingColumns,
  getEmployeeByEmail,
  createPerformanceRating,
  listReviewCycles,
  createReviewCycle,
} from '../../lib/tauri-commands';
import { useImportPipeline } from '../../hooks/useImportPipeline';
import type { ImportResultCommon } from '../../hooks/useImportPipeline';

// Standard field labels for display
const RATING_FIELD_LABELS: Record<string, string> = {
  employee_email: 'Employee Email',
  rating: 'Overall Rating',
  goals_rating: 'Goals Rating',
  competencies_rating: 'Competencies Rating',
  rated_at: 'Rating Date',
  notes: 'Notes',
};

// Required fields for ratings import
const REQUIRED_FIELDS = ['employee_email', 'rating'];

interface RatingsImportProps {
  /** Callback when import completes successfully */
  onComplete?: (result: ImportResultCommon) => void;
  /** Callback when user cancels */
  onCancel?: () => void;
}

export function RatingsImport({ onComplete, onCancel }: RatingsImportProps) {
  // Review cycle selection state (pre-pipeline step)
  const [cycleStep, setCycleStep] = useState<'select-cycle' | 'pipeline'>('select-cycle');
  const [reviewCycles, setReviewCycles] = useState<ReviewCycle[]>([]);
  const [selectedCycle, setSelectedCycle] = useState<ReviewCycle | null>(null);
  const [newCycleName, setNewCycleName] = useState('');
  const [cycleError, setCycleError] = useState<string | null>(null);

  // Import function that transforms rows and calls backend
  async function doImport(
    rows: ParsedRow[],
    mapping: ColumnMapping,
    _resolutions: DuplicateResolution[]
  ): Promise<ImportResultCommon> {
    if (!selectedCycle) {
      throw new Error('No review cycle selected');
    }

    const importResult: ImportResultCommon = { created: 0, skipped: 0, errors: [] };

    for (const row of rows) {
      try {
        const rating = await transformRowToRating(row, mapping, selectedCycle.id);
        if (rating) {
          await createPerformanceRating(rating);
          importResult.created++;
        } else {
          importResult.skipped = (importResult.skipped ?? 0) + 1;
        }
      } catch (err) {
        const email = row[mapping.employee_email] || 'unknown';
        importResult.errors.push(`${email}: ${err instanceof Error ? err.message : 'Failed'}`);
      }
    }

    if (importResult.created === 0 && importResult.errors.length === 0) {
      throw new Error('No valid rating records found in file');
    }

    return importResult;
  }

  const [state, actions] = useImportPipeline({
    dataType: 'ratings',
    targetFields: RATING_FIELD_LABELS,
    requiredFields: REQUIRED_FIELDS,
    autoMapFn: mapRatingColumns,
    importFn: doImport,
    onComplete,
    onCancel,
  });

  // Load review cycles on mount
  useEffect(() => {
    loadReviewCycles();
  }, []);

  const loadReviewCycles = async () => {
    try {
      const cycles = await listReviewCycles();
      setReviewCycles(cycles);
    } catch {
      setCycleError('Failed to load review cycles');
    }
  };

  const handleCreateCycle = async () => {
    if (!newCycleName.trim()) return;

    try {
      const today = new Date();
      const yearStart = new Date(today.getFullYear(), 0, 1).toISOString().split('T')[0];
      const yearEnd = new Date(today.getFullYear(), 11, 31).toISOString().split('T')[0];

      const cycle = await createReviewCycle({
        name: newCycleName.trim(),
        cycle_type: 'annual',
        start_date: yearStart,
        end_date: yearEnd,
        status: 'active',
      });

      setSelectedCycle(cycle);
      setReviewCycles((prev) => [cycle, ...prev]);
      setCycleStep('pipeline');
      setNewCycleName('');
    } catch {
      setCycleError('Failed to create review cycle');
    }
  };

  const handleSelectCycle = (cycle: ReviewCycle) => {
    setSelectedCycle(cycle);
    setCycleStep('pipeline');
  };

  const handleChangeCycle = () => {
    setCycleStep('select-cycle');
    actions.reset();
  };

  const progressKey = getProgressStepKey(state.step);

  // Cycle selection step (pre-pipeline)
  if (cycleStep === 'select-cycle') {
    return (
      <CycleSelector
        cycles={reviewCycles}
        newCycleName={newCycleName}
        onNewCycleNameChange={setNewCycleName}
        onCreateCycle={handleCreateCycle}
        onSelectCycle={handleSelectCycle}
        onCancel={onCancel}
        error={cycleError}
      />
    );
  }

  // Complete screen
  if (state.step === 'complete' && state.importResult) {
    return (
      <ImportResultView
        result={state.importResult}
        onDone={() => {
          actions.reset();
          setCycleStep('select-cycle');
          setSelectedCycle(null);
        }}
      />
    );
  }

  return (
    <div className="space-y-4">
      {/* Cycle Header */}
      {selectedCycle && (
        <CycleHeader cycle={selectedCycle} onChangeCycle={handleChangeCycle} />
      )}

      {/* Step Progress */}
      {state.step !== 'upload' && (
        <StepProgress steps={IMPORT_STEPS} currentStep={progressKey} />
      )}

      {/* Error Banner */}
      {state.error && <ErrorBanner message={state.error} />}

      {/* Upload step */}
      {state.step === 'upload' && (
        <>
          <FileDropzone
            onFileSelect={actions.selectFile}
            isLoading={state.isProcessing}
          />
          <ImportInstructions />
        </>
      )}

      {/* Column Mapping step */}
      {state.step === 'mapping' && (
        <ColumnMappingStep
          sourceHeaders={state.sourceHeaders}
          targetFields={RATING_FIELD_LABELS}
          requiredFields={REQUIRED_FIELDS}
          initialMapping={state.columnMapping}
          normalizations={state.normalizations}
          dataType="ratings"
          onConfirm={actions.confirmMapping}
          onBack={actions.goBack}
        />
      )}

      {/* Validating spinner */}
      {state.step === 'validating' && (
        <div className="flex flex-col items-center justify-center py-12">
          <svg
            className="w-8 h-8 text-primary-500 animate-spin"
            fill="none"
            viewBox="0 0 24 24"
          >
            <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
            <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z" />
          </svg>
          <p className="mt-3 text-sm text-stone-600">Validating data...</p>
        </div>
      )}

      {/* Validation Review step */}
      {state.step === 'validation-review' && state.validationResult && (
        <ValidationStep
          validationResult={state.validationResult}
          totalRows={state.allRows.length}
          onFixIssues={() => {
            actions.fixAndRevalidate(state.allRows);
          }}
          onContinue={actions.skipErrors}
          onBack={actions.goBack}
        />
      )}

      {/* Fix and Retry step */}
      {state.step === 'fixing' && (
        <FixAndRetryStep
          issueRows={state.issueRows}
          allRows={state.allRows}
          columnMapping={state.columnMapping}
          fieldLabels={RATING_FIELD_LABELS}
          onRevalidate={actions.fixAndRevalidate}
          onSkipErrors={actions.skipErrors}
          onBack={actions.goBack}
        />
      )}

      {/* Dedupe step */}
      {state.step === 'deduping' && !state.isProcessing && state.duplicates.length > 0 && (
        <DedupeStep
          duplicates={state.duplicates}
          onResolve={actions.resolveDuplicates}
          onBack={actions.goBack}
        />
      )}

      {/* Dedupe loading */}
      {state.step === 'deduping' && state.isProcessing && (
        <div className="flex flex-col items-center justify-center py-12">
          <svg
            className="w-8 h-8 text-primary-500 animate-spin"
            fill="none"
            viewBox="0 0 24 24"
          >
            <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
            <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z" />
          </svg>
          <p className="mt-3 text-sm text-stone-600">Checking for duplicates...</p>
        </div>
      )}

      {/* Preview step */}
      {(state.step === 'preview' || state.step === 'importing') && state.preview && (
        <ImportPreview
          preview={{ ...state.preview, total_rows: state.allRows.length }}
          columnMapping={state.columnMapping}
          requiredFields={REQUIRED_FIELDS}
          fieldLabels={RATING_FIELD_LABELS}
          onImport={actions.startImport}
          onCancel={actions.goBack}
          isImporting={state.step === 'importing'}
        />
      )}
    </div>
  );
}

// =============================================================================
// Data Transformation
// =============================================================================

/**
 * Transform a parsed row into a CreateRatingInput.
 */
async function transformRowToRating(
  row: ParsedRow,
  mapping: ColumnMapping,
  reviewCycleId: string
): Promise<CreateRatingInput | null> {
  const getValue = (field: string): string | undefined => {
    const header = mapping[field];
    return header ? row[header]?.trim() : undefined;
  };

  const email = getValue('employee_email');
  const ratingStr = getValue('rating');

  if (!email || !ratingStr) return null;

  // Look up employee by email
  const employee = await getEmployeeByEmail(email);
  if (!employee) {
    throw new Error(`Employee not found: ${email}`);
  }

  // Parse rating (1.0 - 5.0)
  const rating = parseFloat(ratingStr);
  if (isNaN(rating) || rating < 1 || rating > 5) {
    throw new Error(`Invalid rating: ${ratingStr} (must be 1.0-5.0)`);
  }

  const goalsStr = getValue('goals_rating');
  const competenciesStr = getValue('competencies_rating');

  return {
    employee_id: employee.id,
    review_cycle_id: reviewCycleId,
    overall_rating: rating,
    goals_rating: goalsStr ? parseFloat(goalsStr) : undefined,
    competencies_rating: competenciesStr ? parseFloat(competenciesStr) : undefined,
    rating_date: getValue('rated_at'),
  };
}

// =============================================================================
// Sub-components
// =============================================================================

function CycleSelector({
  cycles,
  newCycleName,
  onNewCycleNameChange,
  onCreateCycle,
  onSelectCycle,
  onCancel,
  error,
}: {
  cycles: ReviewCycle[];
  newCycleName: string;
  onNewCycleNameChange: (name: string) => void;
  onCreateCycle: () => void;
  onSelectCycle: (cycle: ReviewCycle) => void;
  onCancel?: () => void;
  error: string | null;
}) {
  return (
    <div className="bg-white rounded-xl border border-stone-200 shadow-sm overflow-hidden">
      <div className="px-6 py-4 border-b border-stone-200 bg-stone-50">
        <h3 className="text-lg font-medium text-stone-900">Select Review Cycle</h3>
        <p className="mt-1 text-sm text-stone-500">
          Choose which review cycle these ratings belong to
        </p>
      </div>

      {error && (
        <div className="px-6 py-3 bg-red-50 border-b border-red-200">
          <p className="text-sm text-red-700">{error}</p>
        </div>
      )}

      <div className="p-6 space-y-4">
        {/* Existing cycles */}
        {cycles.length > 0 && (
          <div>
            <h4 className="text-sm font-medium text-stone-700 mb-2">Existing Cycles</h4>
            <div className="space-y-2">
              {cycles.map((cycle) => (
                <button
                  key={cycle.id}
                  onClick={() => onSelectCycle(cycle)}
                  className="w-full p-3 text-left border border-stone-200 rounded-lg hover:border-primary-300 hover:bg-primary-50 transition-colors"
                >
                  <div className="font-medium text-stone-900">{cycle.name}</div>
                  <div className="text-sm text-stone-500">
                    {cycle.cycle_type} • {cycle.start_date} to {cycle.end_date}
                    <span
                      className={`ml-2 px-1.5 py-0.5 rounded text-xs ${
                        cycle.status === 'active'
                          ? 'bg-green-100 text-green-700'
                          : 'bg-stone-100 text-stone-600'
                      }`}
                    >
                      {cycle.status}
                    </span>
                  </div>
                </button>
              ))}
            </div>
          </div>
        )}

        {/* Create new cycle */}
        <div className="pt-4 border-t border-stone-200">
          <h4 className="text-sm font-medium text-stone-700 mb-2">Or Create New Cycle</h4>
          <div className="flex gap-2">
            <input
              type="text"
              value={newCycleName}
              onChange={(e) => onNewCycleNameChange(e.target.value)}
              placeholder="e.g., 2024 Annual Review"
              className="flex-1 px-3 py-2 border border-stone-300 rounded-lg focus:outline-none focus:ring-2 focus:ring-primary-200 focus:border-primary-400"
            />
            <button
              onClick={onCreateCycle}
              disabled={!newCycleName.trim()}
              className={`px-4 py-2 rounded-lg font-medium transition-colors ${
                newCycleName.trim()
                  ? 'bg-primary-500 text-white hover:bg-primary-600'
                  : 'bg-stone-200 text-stone-400 cursor-not-allowed'
              }`}
            >
              Create
            </button>
          </div>
        </div>
      </div>

      {onCancel && (
        <div className="px-6 py-4 border-t border-stone-200 bg-stone-50">
          <button
            onClick={onCancel}
            className="px-4 py-2 text-sm font-medium text-stone-700 bg-white border border-stone-300 rounded-lg hover:bg-stone-50"
          >
            Cancel
          </button>
        </div>
      )}
    </div>
  );
}

function CycleHeader({ cycle, onChangeCycle }: { cycle: ReviewCycle; onChangeCycle: () => void }) {
  return (
    <div className="flex items-center justify-between p-4 bg-primary-50 border border-primary-200 rounded-lg">
      <div>
        <div className="text-sm text-primary-600 font-medium">Review Cycle</div>
        <div className="text-lg font-semibold text-primary-900">{cycle.name}</div>
      </div>
      <button
        onClick={onChangeCycle}
        className="text-sm text-primary-600 hover:text-primary-700 hover:underline"
      >
        Change
      </button>
    </div>
  );
}

function ImportResultView({
  result,
  onDone,
}: {
  result: ImportResultCommon;
  onDone: () => void;
}) {
  const hasErrors = result.errors.length > 0;
  const total = result.created + (result.skipped ?? 0);

  return (
    <div className="bg-white rounded-xl border border-stone-200 shadow-sm overflow-hidden">
      <div className="px-6 py-8 text-center">
        <div
          className={`
            w-16 h-16 mx-auto mb-4
            flex items-center justify-center
            rounded-full
            ${hasErrors ? 'bg-warning/10' : 'bg-primary-50'}
          `}
        >
          {hasErrors ? (
            <svg className="w-8 h-8 text-warning" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth={2}
                d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z"
              />
            </svg>
          ) : (
            <svg className="w-8 h-8 text-primary-500" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth={2}
                d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z"
              />
            </svg>
          )}
        </div>

        <h3 className="text-xl font-semibold text-stone-900">
          {hasErrors ? 'Import Completed with Warnings' : 'Import Successful'}
        </h3>

        <p className="mt-2 text-stone-600">
          {total} rating{total !== 1 ? 's' : ''} processed
        </p>

        <div className="mt-6 flex justify-center gap-8 text-sm">
          <div className="text-center">
            <div className="text-2xl font-bold text-primary-600">{result.created}</div>
            <div className="text-stone-500">Ratings Created</div>
          </div>
          <div className="text-center">
            <div className="text-2xl font-bold text-stone-600">{result.skipped ?? 0}</div>
            <div className="text-stone-500">Skipped</div>
          </div>
          {hasErrors && (
            <div className="text-center">
              <div className="text-2xl font-bold text-warning">{result.errors.length}</div>
              <div className="text-stone-500">Errors</div>
            </div>
          )}
        </div>

        {hasErrors && (
          <div className="mt-6 p-4 bg-amber-50 rounded-lg text-left max-h-40 overflow-y-auto">
            <h4 className="text-sm font-medium text-amber-800 mb-2">Errors:</h4>
            <ul className="text-sm text-amber-700 space-y-1">
              {result.errors.slice(0, 10).map((err, idx) => (
                <li key={idx}>&#8226; {err}</li>
              ))}
              {result.errors.length > 10 && (
                <li className="text-amber-600">
                  ...and {result.errors.length - 10} more
                </li>
              )}
            </ul>
          </div>
        )}
      </div>

      <div className="px-6 py-4 border-t border-stone-200 bg-stone-50">
        <button
          onClick={onDone}
          className="w-full py-2 px-4 bg-primary-500 hover:bg-primary-600 text-white font-medium rounded-lg transition-colors"
        >
          Done
        </button>
      </div>
    </div>
  );
}

function ErrorBanner({ message }: { message: string }) {
  return (
    <div className="p-4 bg-red-50 border border-red-200 rounded-lg flex items-start gap-3 animate-fade-in">
      <svg className="w-5 h-5 text-red-500 flex-shrink-0 mt-0.5" fill="currentColor" viewBox="0 0 20 20">
        <path
          fillRule="evenodd"
          d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-7 4a1 1 0 11-2 0 1 1 0 012 0zm-1-9a1 1 0 00-1 1v4a1 1 0 102 0V6a1 1 0 00-1-1z"
          clipRule="evenodd"
        />
      </svg>
      <div className="flex-1">
        <h4 className="text-sm font-medium text-red-800">Import Error</h4>
        <p className="mt-1 text-sm text-red-700">{message}</p>
      </div>
    </div>
  );
}

function ImportInstructions() {
  return (
    <div className="p-4 bg-stone-50 rounded-lg border border-stone-200">
      <h4 className="text-sm font-medium text-stone-700 mb-2">Expected Columns</h4>
      <p className="text-sm text-stone-600 mb-3">
        Your file should include <strong>employee email</strong> and <strong>rating</strong> (1-5 scale).
      </p>
      <div className="flex flex-wrap gap-2">
        {Object.entries(RATING_FIELD_LABELS).map(([field, label]) => (
          <span
            key={field}
            className={`
              inline-block px-2 py-1 text-xs rounded
              ${REQUIRED_FIELDS.includes(field)
                ? 'bg-primary-100 text-primary-700 font-medium'
                : 'bg-stone-200 text-stone-600'}
            `}
          >
            {label}
          </span>
        ))}
      </div>
    </div>
  );
}

export default RatingsImport;
