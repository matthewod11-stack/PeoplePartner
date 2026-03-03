// People Partner - Onboarding Step Indicator
// Visual progress dots showing wizard position

import { useOnboarding, ONBOARDING_STEPS } from './OnboardingContext';

interface StepIndicatorProps {
  /** Allow clicking dots to navigate */
  interactive?: boolean;
}

export function StepIndicator({ interactive = false }: StepIndicatorProps) {
  const { currentStep, isStepCompleted, goToStep } = useOnboarding();

  return (
    <div className="flex items-center justify-center gap-2" role="progressbar" aria-valuenow={currentStep} aria-valuemin={1} aria-valuemax={7}>
      {ONBOARDING_STEPS.map((step) => {
        const isCurrent = step.number === currentStep;
        const isCompleted = isStepCompleted(step.number);
        const isPast = step.number < currentStep;

        return (
          <button
            key={step.number}
            type="button"
            onClick={() => interactive && goToStep(step.number)}
            disabled={!interactive}
            aria-label={`Step ${step.number}: ${step.name}${isCurrent ? ' (current)' : ''}${isCompleted ? ' (completed)' : ''}`}
            className={`
              w-2.5 h-2.5 rounded-full
              transition-all duration-300
              ${interactive ? 'cursor-pointer hover:scale-125' : 'cursor-default'}
              ${
                isCurrent
                  ? 'bg-primary-500 scale-125 ring-4 ring-primary-100'
                  : isCompleted || isPast
                  ? 'bg-primary-400'
                  : 'bg-stone-200'
              }
            `}
          />
        );
      })}
    </div>
  );
}

export default StepIndicator;
