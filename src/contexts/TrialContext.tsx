// HR Command Center - Trial Mode Context
// Manages trial status, upgrade prompts, and limit tracking

import {
  createContext,
  useContext,
  useState,
  useCallback,
  useEffect,
  useMemo,
  type ReactNode,
} from 'react';
import { getTrialStatus, type TrialStatus } from '../lib/tauri-commands';

// =============================================================================
// Types
// =============================================================================

type UpgradePromptSeverity = 'soft' | 'hard';

interface TrialContextValue {
  /** Full trial status from backend */
  trialStatus: TrialStatus | null;
  /** Whether the app is in trial mode */
  isTrialMode: boolean;
  /** Messages remaining before limit */
  messagesRemaining: number;
  /** Whether the message limit has been reached */
  isAtMessageLimit: boolean;
  /** Whether the employee limit has been reached */
  isAtEmployeeLimit: boolean;
  /** Whether the upgrade prompt should be shown */
  showUpgradePrompt: boolean;
  /** Severity of the upgrade prompt */
  promptSeverity: UpgradePromptSeverity;
  /** Dismiss the upgrade prompt (soft only) */
  dismissUpgradePrompt: () => void;
  /** Re-fetch trial status from backend */
  refreshTrialStatus: () => Promise<void>;
  /** Open the upgrade prompt manually */
  triggerUpgradePrompt: (severity: UpgradePromptSeverity) => void;
}

// =============================================================================
// Context
// =============================================================================

const TrialContext = createContext<TrialContextValue | null>(null);

// =============================================================================
// Constants
// =============================================================================

/** Show soft prompt when this many messages remain */
const SOFT_PROMPT_THRESHOLD = 5;

/** Session storage key for banner dismissal */
const BANNER_DISMISSED_KEY = 'hrcommand_trial_banner_dismissed';

/** Session storage key for soft prompt dismissal (don't re-show same session) */
const SOFT_PROMPT_DISMISSED_KEY = 'hrcommand_upgrade_prompt_dismissed';

// =============================================================================
// Provider
// =============================================================================

interface TrialProviderProps {
  children: ReactNode;
}

export function TrialProvider({ children }: TrialProviderProps) {
  const [trialStatus, setTrialStatus] = useState<TrialStatus | null>(null);
  const [showUpgradePrompt, setShowUpgradePrompt] = useState(false);
  const [promptSeverity, setPromptSeverity] = useState<UpgradePromptSeverity>('soft');

  // Derived state
  const isTrialMode = trialStatus?.is_trial ?? false;
  const messagesRemaining = trialStatus
    ? trialStatus.messages_limit - trialStatus.messages_used
    : 0;
  const isAtMessageLimit = isTrialMode && messagesRemaining <= 0;
  const isAtEmployeeLimit = isTrialMode && trialStatus
    ? trialStatus.employees_used >= trialStatus.employees_limit
    : false;

  // Fetch trial status
  const refreshTrialStatus = useCallback(async () => {
    try {
      const status = await getTrialStatus();
      setTrialStatus(status);

      // Check if we need to show upgrade prompt
      if (status.is_trial) {
        const remaining = status.messages_limit - status.messages_used;
        const softDismissed = sessionStorage.getItem(SOFT_PROMPT_DISMISSED_KEY) === 'true';

        if (remaining <= 0 || status.employees_used >= status.employees_limit) {
          // Hard prompt - always show at limit
          setPromptSeverity('hard');
          setShowUpgradePrompt(true);
        } else if (remaining <= SOFT_PROMPT_THRESHOLD && !softDismissed) {
          // Soft prompt - show once per session
          setPromptSeverity('soft');
          setShowUpgradePrompt(true);
        }
      }
    } catch (err) {
      console.error('[Trial] Failed to fetch trial status:', err);
      // On error, assume not in trial mode (fail open)
    }
  }, []);

  // Fetch on mount
  useEffect(() => {
    refreshTrialStatus();
  }, [refreshTrialStatus]);

  const dismissUpgradePrompt = useCallback(() => {
    if (promptSeverity === 'soft' || !isTrialMode) {
      // Soft prompts can always be dismissed.
      // Hard prompts can be dismissed once the user has left trial mode (completed upgrade).
      setShowUpgradePrompt(false);
      sessionStorage.setItem(SOFT_PROMPT_DISMISSED_KEY, 'true');
    }
  }, [promptSeverity, isTrialMode]);

  const triggerUpgradePrompt = useCallback((severity: UpgradePromptSeverity) => {
    setPromptSeverity(severity);
    setShowUpgradePrompt(true);
  }, []);

  const value = useMemo<TrialContextValue>(
    () => ({
      trialStatus,
      isTrialMode,
      messagesRemaining,
      isAtMessageLimit,
      isAtEmployeeLimit,
      showUpgradePrompt,
      promptSeverity,
      dismissUpgradePrompt,
      refreshTrialStatus,
      triggerUpgradePrompt,
    }),
    [
      trialStatus,
      isTrialMode,
      messagesRemaining,
      isAtMessageLimit,
      isAtEmployeeLimit,
      showUpgradePrompt,
      promptSeverity,
      dismissUpgradePrompt,
      refreshTrialStatus,
      triggerUpgradePrompt,
    ]
  );

  return (
    <TrialContext.Provider value={value}>
      {children}
    </TrialContext.Provider>
  );
}

// =============================================================================
// Hook
// =============================================================================

export function useTrial() {
  const context = useContext(TrialContext);
  if (!context) {
    throw new Error('useTrial must be used within a TrialProvider');
  }
  return context;
}

// Re-export for convenience
export { BANNER_DISMISSED_KEY };
