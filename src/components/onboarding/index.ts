// People Partner - Onboarding Components
// Export all onboarding-related components

export { OnboardingProvider, useOnboarding, ONBOARDING_STEPS, SETTINGS_KEYS } from './OnboardingContext';
export type { OnboardingStep, StepInfo } from './OnboardingContext';

export { OnboardingFlow } from './OnboardingFlow';
export { StepIndicator } from './StepIndicator';

// Step components (typically used internally by OnboardingFlow)
export { WelcomeStep } from './steps/WelcomeStep';
export { ApiKeyStep } from './steps/ApiKeyStep';
export { CompanyStep } from './steps/CompanyStep';
export { EmployeeImportStep } from './steps/EmployeeImportStep';
export { DisclaimerStep } from './steps/DisclaimerStep';
export { TelemetryStep } from './steps/TelemetryStep';
export { FirstPromptStep } from './steps/FirstPromptStep';
