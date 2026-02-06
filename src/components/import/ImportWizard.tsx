import { useState, useCallback } from 'react';
import { EmployeeImport } from './EmployeeImport';
import { RatingsImport } from './RatingsImport';
import { ReviewsImport } from './ReviewsImport';
import { EnpsImport } from './EnpsImport';
import { Modal } from '../shared/Modal';

// Common import result type (supports both created/updated and created/skipped patterns)
interface ImportResultCommon {
  created: number;
  updated?: number;
  skipped?: number;
  errors: string[];
}

// =============================================================================
// Types
// =============================================================================

type ImportType = 'employees' | 'ratings' | 'reviews' | 'enps';
type WizardStep = 'select' | 'import' | 'complete';

interface ImportWizardProps {
  isOpen: boolean;
  onClose: () => void;
  onComplete?: () => void;
}

// =============================================================================
// Data Type Options
// =============================================================================

const DATA_TYPES: {
  id: ImportType;
  title: string;
  description: string;
  icon: React.ReactNode;
}[] = [
  {
    id: 'employees',
    title: 'Employees',
    description: 'Import employee roster with demographics and contact info',
    icon: (
      <svg className="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5} aria-hidden="true">
        <path strokeLinecap="round" strokeLinejoin="round" d="M15 19.128a9.38 9.38 0 002.625.372 9.337 9.337 0 004.121-.952 4.125 4.125 0 00-7.533-2.493M15 19.128v-.003c0-1.113-.285-2.16-.786-3.07M15 19.128v.106A12.318 12.318 0 018.624 21c-2.331 0-4.512-.645-6.374-1.766l-.001-.109a6.375 6.375 0 0111.964-3.07M12 6.375a3.375 3.375 0 11-6.75 0 3.375 3.375 0 016.75 0zm8.25 2.25a2.625 2.625 0 11-5.25 0 2.625 2.625 0 015.25 0z" />
      </svg>
    ),
  },
  {
    id: 'ratings',
    title: 'Performance Ratings',
    description: 'Import numeric performance ratings for review cycles',
    icon: (
      <svg className="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5} aria-hidden="true">
        <path strokeLinecap="round" strokeLinejoin="round" d="M11.48 3.499a.562.562 0 011.04 0l2.125 5.111a.563.563 0 00.475.345l5.518.442c.499.04.701.663.321.988l-4.204 3.602a.563.563 0 00-.182.557l1.285 5.385a.562.562 0 01-.84.61l-4.725-2.885a.563.563 0 00-.586 0L6.982 20.54a.562.562 0 01-.84-.61l1.285-5.386a.562.562 0 00-.182-.557l-4.204-3.602a.563.563 0 01.321-.988l5.518-.442a.563.563 0 00.475-.345L11.48 3.5z" />
      </svg>
    ),
  },
  {
    id: 'reviews',
    title: 'Performance Reviews',
    description: 'Import written review narratives and feedback',
    icon: (
      <svg className="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5} aria-hidden="true">
        <path strokeLinecap="round" strokeLinejoin="round" d="M19.5 14.25v-2.625a3.375 3.375 0 00-3.375-3.375h-1.5A1.125 1.125 0 0113.5 7.125v-1.5a3.375 3.375 0 00-3.375-3.375H8.25m0 12.75h7.5m-7.5 3H12M10.5 2.25H5.625c-.621 0-1.125.504-1.125 1.125v17.25c0 .621.504 1.125 1.125 1.125h12.75c.621 0 1.125-.504 1.125-1.125V11.25a9 9 0 00-9-9z" />
      </svg>
    ),
  },
  {
    id: 'enps',
    title: 'eNPS Responses',
    description: 'Import employee Net Promoter Score survey results',
    icon: (
      <svg className="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5} aria-hidden="true">
        <path strokeLinecap="round" strokeLinejoin="round" d="M3 13.125C3 12.504 3.504 12 4.125 12h2.25c.621 0 1.125.504 1.125 1.125v6.75C7.5 20.496 6.996 21 6.375 21h-2.25A1.125 1.125 0 013 19.875v-6.75zM9.75 8.625c0-.621.504-1.125 1.125-1.125h2.25c.621 0 1.125.504 1.125 1.125v11.25c0 .621-.504 1.125-1.125 1.125h-2.25a1.125 1.125 0 01-1.125-1.125V8.625zM16.5 4.125c0-.621.504-1.125 1.125-1.125h2.25C20.496 3 21 3.504 21 4.125v15.75c0 .621-.504 1.125-1.125 1.125h-2.25a1.125 1.125 0 01-1.125-1.125V4.125z" />
      </svg>
    ),
  },
];

// =============================================================================
// Helper Components
// =============================================================================

interface DataTypeCardProps {
  title: string;
  description: string;
  icon: React.ReactNode;
  isSelected: boolean;
  onClick: () => void;
}

function DataTypeCard({ title, description, icon, isSelected, onClick }: DataTypeCardProps) {
  return (
    <button
      onClick={onClick}
      className={`
        w-full p-4 rounded-xl text-left transition-all duration-200
        ${isSelected
          ? 'bg-primary-50 border-2 border-primary-400 shadow-sm'
          : 'bg-white border-2 border-stone-200 hover:border-stone-300 hover:shadow-sm'}
      `}
    >
      <div className="flex items-start gap-3">
        <div className={`
          p-2 rounded-lg flex-shrink-0
          ${isSelected ? 'bg-primary-100 text-primary-600' : 'bg-stone-100 text-stone-500'}
        `}>
          {icon}
        </div>
        <div>
          <h3 className={`font-medium ${isSelected ? 'text-primary-700' : 'text-stone-800'}`}>
            {title}
          </h3>
          <p className="text-sm text-stone-500 mt-0.5">{description}</p>
        </div>
        {isSelected && (
          <svg className="w-5 h-5 text-primary-500 ml-auto flex-shrink-0" fill="currentColor" viewBox="0 0 20 20" aria-hidden="true">
            <path fillRule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zm3.857-9.809a.75.75 0 00-1.214-.882l-3.483 4.79-1.88-1.88a.75.75 0 10-1.06 1.061l2.5 2.5a.75.75 0 001.137-.089l4-5.5z" clipRule="evenodd" />
          </svg>
        )}
      </div>
    </button>
  );
}

function SuccessScreen({ result, onDone }: { result: ImportResultCommon; onDone: () => void }) {
  return (
    <div className="text-center py-8" role="status" aria-live="polite">
      <div className="w-16 h-16 mx-auto mb-4 bg-emerald-100 rounded-full flex items-center justify-center">
        <svg className="w-8 h-8 text-emerald-600" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2} aria-hidden="true">
          <path strokeLinecap="round" strokeLinejoin="round" d="M5 13l4 4L19 7" />
        </svg>
      </div>
      <h3 className="text-lg font-medium text-stone-800 mb-2">Import Complete</h3>
      <p className="text-stone-600 mb-4">
        {result.created > 0 && <span className="block">{result.created} records created</span>}
        {result.updated && result.updated > 0 && <span className="block">{result.updated} records updated</span>}
        {result.skipped && result.skipped > 0 && <span className="block">{result.skipped} records skipped</span>}
      </p>
      {result.errors.length > 0 && (
        <div className="text-left mb-4 p-3 bg-amber-50 rounded-lg border border-amber-200">
          <p className="text-sm font-medium text-amber-700 mb-1">
            {result.errors.length} warning{result.errors.length !== 1 ? 's' : ''}
          </p>
          <ul className="text-xs text-amber-600 list-disc list-inside max-h-24 overflow-y-auto">
            {result.errors.slice(0, 5).map((err, i) => (
              <li key={i}>{err}</li>
            ))}
            {result.errors.length > 5 && (
              <li>...and {result.errors.length - 5} more</li>
            )}
          </ul>
        </div>
      )}
      <button
        onClick={onDone}
        className="px-6 py-2 bg-primary-500 hover:bg-primary-600 text-white font-medium rounded-lg transition-colors"
      >
        Done
      </button>
    </div>
  );
}

// =============================================================================
// Main Component
// =============================================================================

export function ImportWizard({ isOpen, onClose, onComplete }: ImportWizardProps) {
  const [step, setStep] = useState<WizardStep>('select');
  const [selectedType, setSelectedType] = useState<ImportType | null>(null);
  const [importResult, setImportResult] = useState<ImportResultCommon | null>(null);

  const handleTypeSelect = useCallback((type: ImportType) => {
    setSelectedType(type);
  }, []);

  const handleStartImport = useCallback(() => {
    if (selectedType) {
      setStep('import');
    }
  }, [selectedType]);

  const handleImportComplete = useCallback((result: ImportResultCommon) => {
    setImportResult(result);
    setStep('complete');
  }, []);

  const handleImportCancel = useCallback(() => {
    setStep('select');
    setSelectedType(null);
  }, []);

  const handleDone = useCallback(() => {
    setStep('select');
    setSelectedType(null);
    setImportResult(null);
    onComplete?.();
    onClose();
  }, [onClose, onComplete]);

  const handleClose = useCallback(() => {
    setStep('select');
    setSelectedType(null);
    setImportResult(null);
    onClose();
  }, [onClose]);

  return (
    <Modal isOpen={isOpen} onClose={handleClose} title="Import Data" maxWidth="max-w-2xl">
      {step === 'select' && (
        <p className="text-sm text-stone-600 mb-4">Choose what you'd like to import</p>
      )}
      {step === 'import' && selectedType && (
        <p className="text-sm text-stone-600 mb-4">
          Importing {DATA_TYPES.find(t => t.id === selectedType)?.title}
        </p>
      )}

      {/* Content */}
      <div className="max-h-[70vh] overflow-y-auto pr-1">
        {step === 'select' && (
          <div className="space-y-3">
            {DATA_TYPES.map((type) => (
              <DataTypeCard
                key={type.id}
                title={type.title}
                description={type.description}
                icon={type.icon}
                isSelected={selectedType === type.id}
                onClick={() => handleTypeSelect(type.id)}
              />
            ))}
          </div>
        )}

        {step === 'import' && selectedType === 'employees' && (
          <EmployeeImport onComplete={handleImportComplete} onCancel={handleImportCancel} />
        )}

        {step === 'import' && selectedType === 'ratings' && (
          <RatingsImport onComplete={handleImportComplete} onCancel={handleImportCancel} />
        )}

        {step === 'import' && selectedType === 'reviews' && (
          <ReviewsImport onComplete={handleImportComplete} onCancel={handleImportCancel} />
        )}

        {step === 'import' && selectedType === 'enps' && (
          <EnpsImport onComplete={handleImportComplete} onCancel={handleImportCancel} />
        )}

        {step === 'complete' && importResult && (
          <SuccessScreen result={importResult} onDone={handleDone} />
        )}
      </div>

      {/* Footer - only show on select step */}
      {step === 'select' && (
        <div className="mt-4 pt-4 border-t border-stone-200 flex items-center justify-between bg-stone-50 px-4 py-3 rounded-lg">
          <p className="text-xs text-stone-600">
            Supports CSV, Excel (.xlsx, .xls), and TSV files
          </p>
          <div className="flex gap-3">
            <button
              onClick={handleClose}
              className="px-4 py-2 text-sm font-medium text-stone-600 hover:text-stone-800 hover:bg-stone-100 rounded-lg transition-colors"
            >
              Cancel
            </button>
            <button
              onClick={handleStartImport}
              disabled={!selectedType}
              className="
                px-4 py-2 text-sm font-medium text-white
                bg-primary-500 hover:bg-primary-600
                rounded-lg transition-all
                disabled:opacity-50 disabled:cursor-not-allowed
              "
            >
              Continue
            </button>
          </div>
        </div>
      )}
    </Modal>
  );
}

export default ImportWizard;
