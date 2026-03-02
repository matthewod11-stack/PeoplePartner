// HR Command Center - Onboarding Context
// Manages onboarding wizard state and persistence across sessions

import {
  createContext,
  useContext,
  useState,
  useCallback,
  useEffect,
  type ReactNode,
} from 'react';
import {
  getSetting,
  setSetting,
  hasAnyProviderApiKey,
  hasCompany,
} from '../../lib/tauri-commands';

// =============================================================================
// Types
// =============================================================================

/** Onboarding step definitions */
export type OnboardingStep = 1 | 2 | 3 | 4 | 5 | 6 | 7;

/** Step metadata for UI display */
export interface StepInfo {
  number: OnboardingStep;
  name: string;
  required: boolean;
}

/** All onboarding steps with metadata */
export const ONBOARDING_STEPS: StepInfo[] = [
  { number: 1, name: 'Welcome', required: false },
  { number: 2, name: 'AI Provider', required: true },
  { number: 3, name: 'Company', required: true },
  { number: 4, name: 'Employees', required: false },
  { number: 5, name: 'Disclaimer', required: true },
  { number: 6, name: 'Telemetry', required: false },
  { number: 7, name: 'Get Started', required: false },
];

/** Settings keys for persistence */
export const SETTINGS_KEYS = {
  ONBOARDING_COMPLETED: 'onboarding_completed',
  ONBOARDING_STEP: 'onboarding_step',
  TELEMETRY_ENABLED: 'telemetry_enabled',
  DISCLAIMER_ACCEPTED: 'disclaimer_accepted',
  DISCLAIMER_ACCEPTED_AT: 'disclaimer_accepted_at',
} as const;

interface OnboardingContextValue {
  // State
  currentStep: OnboardingStep;
  completedSteps: Set<OnboardingStep>;
  isLoading: boolean;
  isCompleted: boolean;

  // Step info
  stepInfo: StepInfo;
  totalSteps: number;
  canGoBack: boolean;
  canGoForward: boolean;

  // Navigation
  goToStep: (step: OnboardingStep) => void;
  goNext: () => void;
  goBack: () => void;

  // Step completion
  completeStep: (step: OnboardingStep) => Promise<void>;
  isStepCompleted: (step: OnboardingStep) => boolean;

  // Onboarding completion
  finishOnboarding: () => Promise<void>;
}

// =============================================================================
// Context
// =============================================================================

const OnboardingContext = createContext<OnboardingContextValue | null>(null);

// =============================================================================
// Provider
// =============================================================================

interface OnboardingProviderProps {
  children: ReactNode;
}

export function OnboardingProvider({ children }: OnboardingProviderProps) {
  const [currentStep, setCurrentStep] = useState<OnboardingStep>(1);
  const [completedSteps, setCompletedSteps] = useState<Set<OnboardingStep>>(new Set());
  const [isLoading, setIsLoading] = useState(true);
  const [isCompleted, setIsCompleted] = useState(false);

  // ---------------------------------------------------------------------------
  // Initialize: Load saved progress on mount
  // ---------------------------------------------------------------------------
  useEffect(() => {
    const initializeOnboarding = async () => {
      try {
        // Check if onboarding is already completed
        const completed = await getSetting(SETTINGS_KEYS.ONBOARDING_COMPLETED);
        if (completed === 'true') {
          setIsCompleted(true);
          setIsLoading(false);
          return;
        }

        // Load saved step (resume point)
        const savedStep = await getSetting(SETTINGS_KEYS.ONBOARDING_STEP);
        let resumeStep: OnboardingStep = 1;

        if (savedStep) {
          const parsed = parseInt(savedStep, 10);
          if (parsed >= 1 && parsed <= 7) {
            resumeStep = parsed as OnboardingStep;
          }
        }

        // Verify required steps that may have been completed externally
        const completed_steps = new Set<OnboardingStep>();

        // Step 2: AI Provider - check if any provider has a key configured
        const hasKey = await hasAnyProviderApiKey();
        if (hasKey) {
          completed_steps.add(2);
        }

        // Step 3: Company - check if already configured
        const hasCompanyProfile = await hasCompany();
        if (hasCompanyProfile) {
          completed_steps.add(3);
        }

        // Step 5: Disclaimer - check if already accepted
        const disclaimerAccepted = await getSetting(SETTINGS_KEYS.DISCLAIMER_ACCEPTED);
        if (disclaimerAccepted === 'true') {
          completed_steps.add(5);
        }

        setCompletedSteps(completed_steps);

        // If resuming, ensure we don't go backward past completed required steps
        // But allow re-visiting optional steps
        setCurrentStep(resumeStep);
      } catch (err) {
        console.error('[Onboarding] Failed to initialize:', err);
        // Start from beginning on error
        setCurrentStep(1);
      } finally {
        setIsLoading(false);
      }
    };

    initializeOnboarding();
  }, []);

  // ---------------------------------------------------------------------------
  // Persist current step whenever it changes
  // ---------------------------------------------------------------------------
  useEffect(() => {
    if (!isLoading && !isCompleted) {
      setSetting(SETTINGS_KEYS.ONBOARDING_STEP, String(currentStep)).catch((err) => {
        console.error('[Onboarding] Failed to save step:', err);
      });
    }
  }, [currentStep, isLoading, isCompleted]);

  // ---------------------------------------------------------------------------
  // Computed values
  // ---------------------------------------------------------------------------
  const stepInfo = ONBOARDING_STEPS[currentStep - 1];
  const totalSteps = ONBOARDING_STEPS.length;
  const canGoBack = currentStep > 1;
  const canGoForward = currentStep < 7;

  // ---------------------------------------------------------------------------
  // Navigation
  // ---------------------------------------------------------------------------
  const goToStep = useCallback((step: OnboardingStep) => {
    if (step >= 1 && step <= 7) {
      setCurrentStep(step);
    }
  }, []);

  const goNext = useCallback(() => {
    if (currentStep < 7) {
      setCurrentStep((prev) => (prev + 1) as OnboardingStep);
    }
  }, [currentStep]);

  const goBack = useCallback(() => {
    if (currentStep > 1) {
      setCurrentStep((prev) => (prev - 1) as OnboardingStep);
    }
  }, [currentStep]);

  // ---------------------------------------------------------------------------
  // Step completion
  // ---------------------------------------------------------------------------
  const completeStep = useCallback(async (step: OnboardingStep) => {
    setCompletedSteps((prev) => new Set([...prev, step]));

    // Persist step-specific settings
    if (step === 5) {
      // Disclaimer accepted
      await setSetting(SETTINGS_KEYS.DISCLAIMER_ACCEPTED, 'true');
      await setSetting(SETTINGS_KEYS.DISCLAIMER_ACCEPTED_AT, new Date().toISOString());
    }
  }, []);

  const isStepCompleted = useCallback(
    (step: OnboardingStep) => completedSteps.has(step),
    [completedSteps]
  );

  // ---------------------------------------------------------------------------
  // Finish onboarding
  // ---------------------------------------------------------------------------
  const finishOnboarding = useCallback(async () => {
    try {
      await setSetting(SETTINGS_KEYS.ONBOARDING_COMPLETED, 'true');
      setIsCompleted(true);
      console.log('[Onboarding] Completed successfully');
    } catch (err) {
      console.error('[Onboarding] Failed to mark as completed:', err);
      throw err;
    }
  }, []);

  // ---------------------------------------------------------------------------
  // Context value
  // ---------------------------------------------------------------------------
  const value: OnboardingContextValue = {
    // State
    currentStep,
    completedSteps,
    isLoading,
    isCompleted,

    // Step info
    stepInfo,
    totalSteps,
    canGoBack,
    canGoForward,

    // Navigation
    goToStep,
    goNext,
    goBack,

    // Step completion
    completeStep,
    isStepCompleted,

    // Onboarding completion
    finishOnboarding,
  };

  return (
    <OnboardingContext.Provider value={value}>
      {children}
    </OnboardingContext.Provider>
  );
}

// =============================================================================
// Hook
// =============================================================================

export function useOnboarding() {
  const context = useContext(OnboardingContext);
  if (!context) {
    throw new Error('useOnboarding must be used within an OnboardingProvider');
  }
  return context;
}

export default OnboardingContext;
