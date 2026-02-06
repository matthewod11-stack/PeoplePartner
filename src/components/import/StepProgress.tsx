/**
 * StepProgress Component
 *
 * Horizontal progress indicator for the import pipeline.
 * Shows current step, completed steps, and future steps.
 */

interface StepDef {
  key: string;
  label: string;
}

interface StepProgressProps {
  steps: StepDef[];
  currentStep: string;
}

export function StepProgress({ steps, currentStep }: StepProgressProps) {
  const currentIndex = steps.findIndex((s) => s.key === currentStep);

  return (
    <nav
      aria-label="Import progress"
      className="mb-6"
    >
      <ol className="flex items-center gap-1">
        {steps.map((step, index) => {
          const isCompleted = index < currentIndex;
          const isCurrent = index === currentIndex;

          return (
            <li key={step.key} className="flex items-center">
              {index > 0 && (
                <div
                  className={`w-6 h-px mx-1 ${
                    isCompleted ? 'bg-primary-400' : 'bg-stone-300'
                  }`}
                  aria-hidden="true"
                />
              )}
              <div
                className={`
                  flex items-center gap-1.5 px-2.5 py-1 rounded-full text-xs font-medium
                  transition-colors duration-200
                  ${
                    isCurrent
                      ? 'bg-primary-100 text-primary-700 ring-1 ring-primary-300'
                      : isCompleted
                        ? 'bg-primary-50 text-primary-500'
                        : 'bg-stone-100 text-stone-400'
                  }
                `}
                aria-current={isCurrent ? 'step' : undefined}
              >
                {isCompleted ? (
                  <svg
                    className="w-3 h-3"
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
                ) : (
                  <span className="w-3 text-center">{index + 1}</span>
                )}
                <span>{step.label}</span>
              </div>
            </li>
          );
        })}
      </ol>
    </nav>
  );
}

/** Standard steps for the import pipeline */
export const IMPORT_STEPS: StepDef[] = [
  { key: 'upload', label: 'Upload' },
  { key: 'mapping', label: 'Map Columns' },
  { key: 'validate', label: 'Validate' },
  { key: 'review', label: 'Review' },
  { key: 'import', label: 'Import' },
];

/** Map pipeline steps to progress step keys */
export function getProgressStepKey(pipelineStep: string): string {
  switch (pipelineStep) {
    case 'upload':
      return 'upload';
    case 'mapping':
      return 'mapping';
    case 'validating':
    case 'validation-review':
    case 'fixing':
      return 'validate';
    case 'deduping':
    case 'preview':
      return 'review';
    case 'importing':
    case 'complete':
      return 'import';
    default:
      return 'upload';
  }
}

export default StepProgress;
