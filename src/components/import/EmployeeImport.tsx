import { FileDropzone } from './FileDropzone';
import { ImportPreview } from './ImportPreview';
import { ColumnMappingStep } from './ColumnMappingStep';
import { ValidationStep } from './ValidationStep';
import { FixAndRetryStep } from './FixAndRetryStep';
import { DedupeStep } from './DedupeStep';
import { StepProgress, IMPORT_STEPS, getProgressStepKey } from './StepProgress';
import type { ColumnMapping, ParsedRow, DuplicateResolution } from '../../lib/types';
import type { CreateEmployeeInput, ImportResult } from '../../lib/tauri-commands';
import { mapEmployeeColumns, importEmployees } from '../../lib/tauri-commands';
import { useImportPipeline } from '../../hooks/useImportPipeline';
import type { ImportResultCommon } from '../../hooks/useImportPipeline';

// Standard field labels for display
const EMPLOYEE_FIELD_LABELS: Record<string, string> = {
  email: 'Email',
  first_name: 'First Name',
  last_name: 'Last Name',
  department: 'Department',
  title: 'Job Title',
  hire_date: 'Hire Date',
  work_state: 'Work State',
  manager_email: 'Manager Email',
  status: 'Status',
  date_of_birth: 'Date of Birth',
  gender: 'Gender',
  ethnicity: 'Ethnicity',
};

// Required fields for employee import
const REQUIRED_FIELDS = ['email'];

interface EmployeeImportProps {
  /** Callback when import completes successfully */
  onComplete?: (result: ImportResult) => void;
  /** Callback when user cancels */
  onCancel?: () => void;
}

// Import function that transforms rows and calls backend
async function doImport(
  rows: ParsedRow[],
  mapping: ColumnMapping,
  _resolutions: DuplicateResolution[]
): Promise<ImportResultCommon> {
  const employees = rows
    .map((row) => transformRowToEmployee(row, mapping))
    .filter((emp): emp is CreateEmployeeInput => emp !== null);

  if (employees.length === 0) {
    throw new Error('No valid employee records found in file');
  }

  const result = await importEmployees(employees);
  return {
    created: result.created,
    updated: result.updated,
    errors: result.errors,
  };
}

export function EmployeeImport({ onComplete, onCancel }: EmployeeImportProps) {
  const [state, actions] = useImportPipeline({
    dataType: 'employees',
    targetFields: EMPLOYEE_FIELD_LABELS,
    requiredFields: REQUIRED_FIELDS,
    autoMapFn: mapEmployeeColumns,
    importFn: doImport,
    onComplete: onComplete as ((result: ImportResultCommon) => void) | undefined,
    onCancel,
  });

  const progressKey = getProgressStepKey(state.step);

  // Complete screen
  if (state.step === 'complete' && state.importResult) {
    return (
      <ImportResultView
        result={state.importResult}
        onDone={actions.reset}
      />
    );
  }

  return (
    <div className="space-y-4">
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
          targetFields={EMPLOYEE_FIELD_LABELS}
          requiredFields={REQUIRED_FIELDS}
          initialMapping={state.columnMapping}
          normalizations={state.normalizations}
          dataType="employees"
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
            // Transition to fixing step is handled via goBack from fixing or direct state
            // For now, go to fixing step by setting step
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
          fieldLabels={EMPLOYEE_FIELD_LABELS}
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
          fieldLabels={EMPLOYEE_FIELD_LABELS}
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
 * Transform a parsed row into a CreateEmployeeInput.
 */
function transformRowToEmployee(
  row: ParsedRow,
  mapping: ColumnMapping
): CreateEmployeeInput | null {
  const getValue = (field: string): string | undefined => {
    const header = mapping[field];
    return header ? row[header]?.trim() : undefined;
  };

  const email = getValue('email');
  if (!email) {
    return null;
  }

  const firstName = getValue('first_name') || '';
  const lastName = getValue('last_name') || '';
  const fullName = [firstName, lastName].filter(Boolean).join(' ') || email.split('@')[0];

  const rawStatus = getValue('status')?.toLowerCase();
  let status: 'active' | 'terminated' | 'leave' | undefined;
  if (rawStatus) {
    if (['active', 'a', 'employed', 'current'].includes(rawStatus)) {
      status = 'active';
    } else if (['terminated', 't', 'term', 'inactive', 'former'].includes(rawStatus)) {
      status = 'terminated';
    } else if (['leave', 'l', 'loa', 'on leave'].includes(rawStatus)) {
      status = 'leave';
    }
  }

  const rawTermReason = getValue('termination_reason')?.toLowerCase();
  let terminationReason: string | undefined;
  if (rawTermReason) {
    if (['voluntary', 'vol', 'resigned', 'quit'].includes(rawTermReason)) {
      terminationReason = 'voluntary';
    } else if (['involuntary', 'invol', 'fired', 'laid off', 'layoff'].includes(rawTermReason)) {
      terminationReason = 'involuntary';
    } else if (['retirement', 'retired'].includes(rawTermReason)) {
      terminationReason = 'retirement';
    } else {
      terminationReason = 'other';
    }
  }

  return {
    email,
    full_name: fullName,
    department: getValue('department'),
    job_title: getValue('title'),
    hire_date: getValue('hire_date'),
    work_state: getValue('work_state'),
    status,
    date_of_birth: getValue('date_of_birth'),
    gender: getValue('gender'),
    ethnicity: getValue('ethnicity'),
    termination_date: getValue('termination_date'),
    termination_reason: terminationReason,
  };
}

// =============================================================================
// Sub-components
// =============================================================================

function ImportResultView({
  result,
  onDone,
}: {
  result: ImportResultCommon;
  onDone: () => void;
}) {
  const hasErrors = result.errors.length > 0;
  const total = result.created + (result.updated ?? 0);

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
          {total} employee{total !== 1 ? 's' : ''} processed
        </p>

        <div className="mt-6 flex justify-center gap-8 text-sm">
          <div className="text-center">
            <div className="text-2xl font-bold text-primary-600">{result.created}</div>
            <div className="text-stone-500">Created</div>
          </div>
          <div className="text-center">
            <div className="text-2xl font-bold text-stone-600">{result.updated ?? 0}</div>
            <div className="text-stone-500">Updated</div>
          </div>
          {hasErrors && (
            <div className="text-center">
              <div className="text-2xl font-bold text-warning">{result.errors.length}</div>
              <div className="text-stone-500">Errors</div>
            </div>
          )}
        </div>

        {hasErrors && (
          <div className="mt-6 p-4 bg-amber-50 rounded-lg text-left">
            <h4 className="text-sm font-medium text-amber-800 mb-2">Errors:</h4>
            <ul className="text-sm text-amber-700 space-y-1">
              {result.errors.slice(0, 5).map((err, idx) => (
                <li key={idx}>&#8226; {err}</li>
              ))}
              {result.errors.length > 5 && (
                <li className="text-amber-600">
                  ...and {result.errors.length - 5} more
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
        Your file should include an <strong>email</strong> column (required). Other columns are optional:
      </p>
      <div className="flex flex-wrap gap-2">
        {Object.entries(EMPLOYEE_FIELD_LABELS).map(([field, label]) => (
          <span
            key={field}
            className={`
              inline-block px-2 py-1 text-xs rounded
              ${field === 'email'
                ? 'bg-primary-100 text-primary-700 font-medium'
                : 'bg-stone-200 text-stone-600'}
            `}
          >
            {label}
          </span>
        ))}
      </div>
      <p className="mt-3 text-xs text-stone-500">
        Existing employees (matched by email) will be updated. New emails will create new records.
      </p>
    </div>
  );
}

export default EmployeeImport;
