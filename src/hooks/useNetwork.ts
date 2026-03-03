// People Partner - Network Status Hook
// Provides reactive network status with instant browser event detection
// and periodic API reachability verification

import { useState, useEffect, useCallback, useRef } from 'react';
import { checkNetworkStatus, NetworkStatus } from '../lib/tauri-commands';

/** Configuration for network polling behavior */
interface UseNetworkOptions {
  /** Interval for periodic backup checks (ms). Default: 30000 (30 seconds) */
  pollInterval?: number;
  /** Whether to check immediately on mount. Default: true */
  checkOnMount?: boolean;
}

/** Return value from the useNetwork hook */
interface UseNetworkResult {
  /** Whether the network is available */
  isOnline: boolean;
  /** Whether the Anthropic API is specifically reachable */
  isApiReachable: boolean;
  /** Error message if offline */
  errorMessage: string | null;
  /** Whether a network check is currently in progress */
  isChecking: boolean;
  /** Manually trigger a network check */
  checkNow: () => Promise<void>;
  /** Timestamp of last successful check */
  lastChecked: Date | null;
}

/**
 * Hook for monitoring network connectivity with maximum responsiveness.
 *
 * Uses a hybrid approach:
 * 1. Browser events (online/offline) for instant detection
 * 2. API reachability checks to verify Claude is actually reachable
 * 3. Periodic backup checks to catch edge cases
 *
 * @example
 * ```tsx
 * const { isOnline, isApiReachable, checkNow } = useNetwork();
 *
 * if (!isOnline) {
 *   return <OfflineIndicator />;
 * }
 * ```
 */
export function useNetwork(options: UseNetworkOptions = {}): UseNetworkResult {
  const { pollInterval = 30000, checkOnMount = true } = options;

  // Network status state
  const [status, setStatus] = useState<NetworkStatus>({
    is_online: navigator.onLine, // Start with browser's best guess
    api_reachable: navigator.onLine, // Assume reachable if browser says online
    error_message: null,
  });
  const [isChecking, setIsChecking] = useState(false);
  const [lastChecked, setLastChecked] = useState<Date | null>(null);

  // Ref to track if component is mounted (prevent state updates after unmount)
  const isMounted = useRef(true);

  // Perform an actual API reachability check
  const performCheck = useCallback(async () => {
    if (!isMounted.current) return;

    setIsChecking(true);
    try {
      const result = await checkNetworkStatus();
      if (isMounted.current) {
        setStatus(result);
        setLastChecked(new Date());
      }
    } catch (error) {
      // If the Tauri invoke itself fails, we're probably offline
      if (isMounted.current) {
        setStatus({
          is_online: false,
          api_reachable: false,
          error_message: 'Failed to check network status',
        });
      }
    } finally {
      if (isMounted.current) {
        setIsChecking(false);
      }
    }
  }, []);

  // Handle browser online event - verify with actual API check
  const handleOnline = useCallback(() => {
    // Immediately show optimistic online state
    setStatus(prev => ({
      ...prev,
      is_online: true,
      error_message: null,
    }));
    // Then verify API is actually reachable
    performCheck();
  }, [performCheck]);

  // Handle browser offline event - instant feedback
  const handleOffline = useCallback(() => {
    setStatus({
      is_online: false,
      api_reachable: false,
      error_message: 'Network connection lost',
    });
  }, []);

  // Set up browser event listeners and periodic checks
  useEffect(() => {
    isMounted.current = true;

    // Listen to browser network events for instant feedback
    window.addEventListener('online', handleOnline);
    window.addEventListener('offline', handleOffline);

    // Initial check on mount
    if (checkOnMount) {
      performCheck();
    }

    // Periodic backup checks
    const intervalId = setInterval(performCheck, pollInterval);

    return () => {
      isMounted.current = false;
      window.removeEventListener('online', handleOnline);
      window.removeEventListener('offline', handleOffline);
      clearInterval(intervalId);
    };
  }, [handleOnline, handleOffline, performCheck, pollInterval, checkOnMount]);

  return {
    isOnline: status.is_online,
    isApiReachable: status.api_reachable,
    errorMessage: status.error_message,
    isChecking,
    checkNow: performCheck,
    lastChecked,
  };
}

export default useNetwork;
