// HR Command Center - Onboarding Flow
// Main wizard container that renders steps and handles navigation

import { useCallback, type ReactNode } from 'react';
import { useOnboarding, type OnboardingStep } from './OnboardingContext';
import { StepIndicator } from './StepIndicator';

// Step components (will be imported as they're created)
import { WelcomeStep } from './steps/WelcomeStep';
import { ApiKeyStep } from './steps/ApiKeyStep';
import { CompanyStep } from './steps/CompanyStep';
import { EmployeeImportStep } from './steps/EmployeeImportStep';
import { DisclaimerStep } from './steps/DisclaimerStep';
import { TelemetryStep } from './steps/TelemetryStep';
import { FirstPromptStep } from './steps/FirstPromptStep';

// =============================================================================
// Types
// =============================================================================

interface OnboardingFlowProps {
  /** Callback when onboarding is completed */
  onComplete?: () => void;
  /** Optional initial prompt to pre-fill (from prompt suggestion click) */
  initialPrompt?: string;
}

interface StepContainerProps {
  children: ReactNode;
  title?: string;
  subtitle?: string;
}

// =============================================================================
// Step Container (shared layout for most steps)
// =============================================================================

function StepContainer({ children, title, subtitle }: StepContainerProps) {
  return (
    <div className="flex flex-col items-center">
      {(title || subtitle) && (
        <div className="text-center mb-8">
          {title && (
            <h1 className="text-2xl font-display font-semibold text-stone-800">
              {title}
            </h1>
          )}
          {subtitle && (
            <p className="mt-2 text-stone-500">{subtitle}</p>
          )}
        </div>
      )}
      {children}
    </div>
  );
}

// =============================================================================
// Main Component
// =============================================================================

export function OnboardingFlow({ onComplete, initialPrompt }: OnboardingFlowProps) {
  const {
    currentStep,
    isLoading,
    canGoBack,
    canGoForward,
    goNext,
    goBack,
    completeStep,
    finishOnboarding,
    stepInfo,
  } = useOnboarding();

  // Step validity state - tracks if each step can proceed
  // (Note: Currently unused - steps auto-advance on completion)
  const setValid = useCallback((_step: OnboardingStep, _valid: boolean) => {
    // Placeholder for future validation logic if needed
  }, []);

  // Handle step completion and navigation
  const handleStepComplete = useCallback(async (step: OnboardingStep) => {
    await completeStep(step);

    if (step === 7) {
      // Final step - finish onboarding
      await finishOnboarding();
      onComplete?.();
    } else {
      // Move to next step
      goNext();
    }
  }, [completeStep, finishOnboarding, goNext, onComplete]);

  // Render current step
  const renderStep = () => {
    switch (currentStep) {
      case 1:
        return (
          <WelcomeStep
            onContinue={() => handleStepComplete(1)}
          />
        );

      case 2:
        return (
          <StepContainer
            title="Connect to Claude"
            subtitle="Your API key is stored securely in macOS Keychain"
          >
            <ApiKeyStep
              onComplete={() => handleStepComplete(2)}
              onValidChange={(valid) => setValid(2, valid)}
            />
          </StepContainer>
        );

      case 3:
        return (
          <StepContainer
            title="Tell us about your company"
            subtitle="This helps Claude understand your legal context"
          >
            <CompanyStep
              onComplete={() => handleStepComplete(3)}
              onValidChange={(valid) => setValid(3, valid)}
            />
          </StepContainer>
        );

      case 4:
        return (
          <StepContainer
            title="Your team data"
            subtitle="Sample data is pre-loaded so you can explore right away"
          >
            <EmployeeImportStep
              onContinue={() => handleStepComplete(4)}
            />
          </StepContainer>
        );

      case 5:
        return (
          <StepContainer
            title="Important reminder"
            subtitle="A quick note before we begin"
          >
            <DisclaimerStep
              onAccept={() => handleStepComplete(5)}
              onValidChange={(valid) => setValid(5, valid)}
            />
          </StepContainer>
        );

      case 6:
        return (
          <StepContainer
            title="Help improve HR Command Center"
            subtitle="Anonymous crash reports help us fix bugs faster"
          >
            <TelemetryStep
              onContinue={() => handleStepComplete(6)}
            />
          </StepContainer>
        );

      case 7:
        return (
          <FirstPromptStep
            onStart={() => {
              handleStepComplete(7);
            }}
            initialPrompt={initialPrompt}
          />
        );

      default:
        return null;
    }
  };

  // Loading state
  if (isLoading) {
    return (
      <div className="min-h-screen flex items-center justify-center bg-gradient-to-br from-stone-50 to-stone-100">
        <div className="flex flex-col items-center gap-4">
          <div className="w-10 h-10 border-4 border-primary-200 border-t-primary-500 rounded-full animate-spin-slow" />
          <p className="text-stone-500">Loading...</p>
        </div>
      </div>
    );
  }

  const showBackButton = canGoBack && currentStep !== 1;
  const showSkipButton = !stepInfo.required && canGoForward && currentStep !== 7;

  return (
    <div className="min-h-screen flex flex-col bg-gradient-to-br from-stone-50 to-stone-100">
      {/* Header with step indicator */}
      <header className="flex-shrink-0 pt-8 pb-4">
        <StepIndicator />
      </header>

      {/* Main content area */}
      <main className="flex-1 flex items-center justify-center px-6 pb-24 overflow-y-auto">
        <div className="w-full max-w-xl my-auto">
          <div className="bg-white rounded-2xl shadow-lg shadow-stone-200/50 p-8 max-h-[calc(100vh-180px)] overflow-y-auto">
            {renderStep()}
          </div>
        </div>
      </main>

      {/* Bottom navigation */}
      {(showBackButton || showSkipButton) && (
        <footer className="fixed bottom-0 left-0 right-0 bg-white/80 backdrop-blur border-t border-stone-200 py-4 px-6">
          <div className="max-w-xl mx-auto flex items-center justify-between">
            {showBackButton ? (
              <button
                type="button"
                onClick={goBack}
                className="flex items-center gap-2 px-4 py-2 text-stone-600 hover:text-stone-800 hover:bg-stone-100 rounded-lg transition-colors"
              >
                <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
                  <path strokeLinecap="round" strokeLinejoin="round" d="M15 19l-7-7 7-7" />
                </svg>
                Back
              </button>
            ) : (
              <div />
            )}

            {showSkipButton && (
              <button
                type="button"
                onClick={goNext}
                className="px-4 py-2 text-stone-500 hover:text-stone-700 hover:bg-stone-100 rounded-lg transition-colors"
              >
                Skip for now
              </button>
            )}
          </div>
        </footer>
      )}
    </div>
  );
}

export default OnboardingFlow;
