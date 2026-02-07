// Trial Banner - persistent amber banner showing trial status
// Appears between header and main content, dismissible per session

import { useState } from 'react';
import { useTrial, BANNER_DISMISSED_KEY } from '../../contexts/TrialContext';

export function TrialBanner() {
  const { isTrialMode, trialStatus, messagesRemaining, triggerUpgradePrompt } = useTrial();
  const [dismissed, setDismissed] = useState(
    () => sessionStorage.getItem(BANNER_DISMISSED_KEY) === 'true'
  );

  if (!isTrialMode || !trialStatus || dismissed) {
    return null;
  }

  const handleDismiss = () => {
    setDismissed(true);
    sessionStorage.setItem(BANNER_DISMISSED_KEY, 'true');
  };

  const handleUpgradeClick = () => {
    triggerUpgradePrompt('soft');
  };

  return (
    <div
      className="
        flex-shrink-0
        flex items-center justify-between
        px-4 py-2
        bg-amber-500
        text-white text-sm
      "
      role="status"
      aria-label="Trial status"
    >
      <div className="flex items-center gap-2">
        <svg
          className="w-4 h-4 flex-shrink-0"
          fill="none"
          viewBox="0 0 24 24"
          stroke="currentColor"
          strokeWidth={2}
          aria-hidden="true"
        >
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"
          />
        </svg>
        <span>
          Free Trial &mdash; {messagesRemaining > 0
            ? `${messagesRemaining} of ${trialStatus.messages_limit} messages remaining`
            : 'No messages remaining'}
        </span>
      </div>

      <div className="flex items-center gap-2">
        <button
          onClick={handleUpgradeClick}
          className="
            px-3 py-1
            bg-white/20 hover:bg-white/30
            rounded-md
            text-sm font-medium
            transition-colors duration-150
          "
        >
          Upgrade
        </button>
        <button
          onClick={handleDismiss}
          className="
            p-1
            hover:bg-white/20
            rounded-md
            transition-colors duration-150
          "
          aria-label="Dismiss trial banner"
        >
          <svg
            className="w-4 h-4"
            fill="none"
            viewBox="0 0 24 24"
            stroke="currentColor"
            strokeWidth={2}
            aria-hidden="true"
          >
            <path strokeLinecap="round" strokeLinejoin="round" d="M6 18L18 6M6 6l12 12" />
          </svg>
        </button>
      </div>
    </div>
  );
}

export default TrialBanner;
