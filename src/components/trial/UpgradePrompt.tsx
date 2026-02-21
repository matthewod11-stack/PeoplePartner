// Upgrade Prompt Modal - soft (dismissible) and hard (blocking) variants
// Triggered at message/employee limit thresholds

import { useCallback } from 'react';
import { Modal } from '../shared/Modal';
import { Button } from '../ui/Button';
import { useTrial } from '../../contexts/TrialContext';
import { UPGRADE_URL } from '../../lib/constants';

async function openUpgradeUrl() {
  try {
    // Use Tauri shell plugin to open external URL
    const { open } = await import('@tauri-apps/plugin-shell');
    await open(UPGRADE_URL);
  } catch {
    // Fallback to window.open
    window.open(UPGRADE_URL, '_blank');
  }
}

export function UpgradePrompt() {
  const {
    showUpgradePrompt,
    promptSeverity,
    dismissUpgradePrompt,
    trialStatus,
    messagesRemaining,
    isAtEmployeeLimit,
  } = useTrial();

  const handleUpgrade = useCallback(() => {
    openUpgradeUrl();
  }, []);

  if (!showUpgradePrompt || !trialStatus) {
    return null;
  }

  const isHard = promptSeverity === 'hard';

  // Determine the reason for hard prompt
  const hardReason = messagesRemaining <= 0
    ? 'message'
    : isAtEmployeeLimit
      ? 'employee'
      : 'message';

  return (
    <Modal
      isOpen={showUpgradePrompt}
      onClose={isHard ? () => {} : dismissUpgradePrompt}
      title={isHard ? 'Trial complete' : 'Running low on trial messages!'}
      maxWidth="max-w-md"
    >
      <div className="space-y-5">
        {/* Status message */}
        {isHard ? (
          <div className="text-sm text-stone-600">
            {hardReason === 'message' ? (
              <p>
                You&apos;ve used all {trialStatus.messages_limit} trial messages.
                Upgrade to continue using HR Command Center with unlimited messages.
              </p>
            ) : (
              <p>
                You&apos;ve reached the {trialStatus.employees_limit}-employee trial limit.
                Upgrade for unlimited employee records.
              </p>
            )}
          </div>
        ) : (
          <div className="text-sm text-stone-600">
            <p>
              You have <span className="font-semibold text-amber-600">{messagesRemaining}</span> messages
              remaining in your free trial.
            </p>
          </div>
        )}

        {/* Pricing card */}
        <div className="p-4 bg-gradient-to-r from-primary-50 to-amber-50 border border-primary-200 rounded-xl">
          <div className="flex items-baseline gap-2 mb-3">
            <span className="text-2xl font-bold text-stone-800">$99</span>
            <span className="text-sm text-stone-500">one-time purchase</span>
          </div>
          <ul className="space-y-2">
            {[
              'Unlimited messages with Claude AI',
              'Unlimited employee records',
              'All analytics and insight features',
              'Cross-conversation memory',
              'Backup and restore',
              'Lifetime updates',
            ].map((benefit) => (
              <li key={benefit} className="flex items-start gap-2 text-sm text-stone-700">
                <svg
                  className="w-4 h-4 text-primary-500 flex-shrink-0 mt-0.5"
                  fill="none"
                  viewBox="0 0 24 24"
                  stroke="currentColor"
                  strokeWidth={2}
                  aria-hidden="true"
                >
                  <path strokeLinecap="round" strokeLinejoin="round" d="M5 13l4 4L19 7" />
                </svg>
                {benefit}
              </li>
            ))}
          </ul>
        </div>

        {/* Actions */}
        <div className="flex gap-3 pt-1">
          {!isHard && (
            <Button
              variant="ghost"
              size="md"
              fullWidth
              onClick={dismissUpgradePrompt}
            >
              Maybe Later
            </Button>
          )}
          <Button
            variant="primary"
            size="md"
            fullWidth
            onClick={handleUpgrade}
          >
            Upgrade Now
          </Button>
        </div>
      </div>
    </Modal>
  );
}

export default UpgradePrompt;
