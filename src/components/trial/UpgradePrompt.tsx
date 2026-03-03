// Upgrade Prompt Modal - multi-step wizard for trial → paid upgrade
// Step 1: Purchase prompt (pricing + "I have a license key" link)
// Step 2: License key entry
// Step 3: Provider picker + API key setup
// Step 4: Success confirmation

import { useState, useCallback, useEffect } from 'react';
import { Modal } from '../shared/Modal';
import { Button } from '../ui/Button';
import { LicenseKeyInput } from '../settings/LicenseKeyInput';
import { ProviderPicker } from '../settings/ProviderPicker';
import { ApiKeyInput } from '../settings/ApiKeyInput';
import { useTrial } from '../../contexts/TrialContext';
import { UPGRADE_URL } from '../../lib/constants';
import {
  getActiveProvider,
  setActiveProvider,
  hasProviderApiKey,
} from '../../lib/tauri-commands';
import { PROVIDER_ORDER } from '../../lib/provider-config';

type WizardStep = 'purchase' | 'license' | 'provider' | 'complete';

async function openUpgradeUrl() {
  try {
    const { open } = await import('@tauri-apps/plugin-shell');
    await open(UPGRADE_URL);
  } catch {
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
    refreshTrialStatus,
  } = useTrial();

  const [wizardStep, setWizardStep] = useState<WizardStep>('purchase');
  const [selectedProvider, setSelectedProvider] = useState('anthropic');
  const [providerKeyStatus, setProviderKeyStatus] = useState<Record<string, boolean>>({});

  // Reset wizard when modal opens
  useEffect(() => {
    if (showUpgradePrompt) {
      setWizardStep('purchase');
      getActiveProvider()
        .then(setSelectedProvider)
        .catch(() => setSelectedProvider('anthropic'));
      loadProviderKeyStatus();
    }
  }, [showUpgradePrompt]);

  const loadProviderKeyStatus = useCallback(async () => {
    const status: Record<string, boolean> = {};
    for (const id of PROVIDER_ORDER) {
      try {
        status[id] = await hasProviderApiKey(id);
      } catch {
        status[id] = false;
      }
    }
    setProviderKeyStatus(status);
  }, []);

  const handleProviderChange = useCallback(async (providerId: string) => {
    const previousId = selectedProvider;
    setSelectedProvider(providerId);
    try {
      await setActiveProvider(providerId);
    } catch {
      setSelectedProvider(previousId);
    }
  }, [selectedProvider]);

  const handleLicenseSaved = useCallback(async () => {
    await refreshTrialStatus();
    setWizardStep('provider');
  }, [refreshTrialStatus]);

  const handleApiKeySaved = useCallback(async () => {
    await refreshTrialStatus();
    await loadProviderKeyStatus();
    setWizardStep('complete');
  }, [refreshTrialStatus, loadProviderKeyStatus]);

  const handleComplete = useCallback(() => {
    dismissUpgradePrompt();
  }, [dismissUpgradePrompt]);

  if (!showUpgradePrompt || !trialStatus) {
    return null;
  }

  const isHard = promptSeverity === 'hard';

  const hardReason = messagesRemaining <= 0
    ? 'message'
    : isAtEmployeeLimit
      ? 'employee'
      : 'message';

  // Wizard step titles
  const stepTitles: Record<WizardStep, string> = {
    purchase: isHard ? 'Trial complete' : 'Running low on trial messages!',
    license: 'Enter your license key',
    provider: 'Choose your AI provider',
    complete: 'You\'re all set!',
  };

  // Allow closing on non-hard prompts or after completing the wizard
  const canClose = !isHard || wizardStep === 'complete';

  return (
    <Modal
      isOpen={showUpgradePrompt}
      onClose={canClose ? handleComplete : () => {}}
      title={stepTitles[wizardStep]}
      maxWidth="max-w-md"
    >
      <div className="space-y-5">
        {/* Step indicator (visible after purchase step) */}
        {wizardStep !== 'purchase' && wizardStep !== 'complete' && (
          <div className="flex items-center gap-2">
            {(['license', 'provider'] as const).map((step, i) => (
              <div key={step} className="flex items-center gap-2">
                <div
                  className={`
                    w-6 h-6 rounded-full flex items-center justify-center text-xs font-medium
                    ${wizardStep === step
                      ? 'bg-primary-500 text-white'
                      : wizardStep === 'provider' && step === 'license'
                        ? 'bg-green-100 text-green-700'
                        : 'bg-stone-100 text-stone-400'
                    }
                  `}
                >
                  {wizardStep === 'provider' && step === 'license' ? (
                    <svg className="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={3}>
                      <path strokeLinecap="round" strokeLinejoin="round" d="M5 13l4 4L19 7" />
                    </svg>
                  ) : (
                    i + 1
                  )}
                </div>
                {i === 0 && (
                  <div className={`w-12 h-0.5 ${wizardStep === 'provider' ? 'bg-green-300' : 'bg-stone-200'}`} />
                )}
              </div>
            ))}
          </div>
        )}

        {/* ─── Step: Purchase ─── */}
        {wizardStep === 'purchase' && (
          <>
            {isHard ? (
              <div className="text-sm text-stone-600">
                {hardReason === 'message' ? (
                  <p>
                    You&apos;ve used all {trialStatus.messages_limit} trial messages.
                    Upgrade to continue using People Partner with unlimited messages.
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
                  'Unlimited messages with your preferred AI',
                  'Unlimited employee records',
                  'Choose from Anthropic, OpenAI, or Google',
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
                onClick={() => openUpgradeUrl()}
              >
                Upgrade Now
              </Button>
            </div>

            {/* "I have a key" link */}
            <div className="text-center pt-1">
              <button
                type="button"
                onClick={() => setWizardStep('license')}
                className="text-sm text-primary-600 hover:text-primary-700 underline underline-offset-2"
              >
                I already have a license key
              </button>
            </div>
          </>
        )}

        {/* ─── Step: License Key ─── */}
        {wizardStep === 'license' && (
          <>
            <p className="text-sm text-stone-600">
              Paste the license key from your purchase confirmation email.
            </p>

            <LicenseKeyInput
              compact
              onSave={handleLicenseSaved}
            />

            <div className="flex gap-3 pt-1">
              <Button
                variant="ghost"
                size="md"
                fullWidth
                onClick={() => setWizardStep('purchase')}
              >
                Back
              </Button>
            </div>
          </>
        )}

        {/* ─── Step: Provider Selection ─── */}
        {wizardStep === 'provider' && (
          <>
            <div className="p-3 bg-green-50 border border-green-200 rounded-lg">
              <div className="flex items-center gap-2">
                <svg className="w-4 h-4 text-green-600 flex-shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
                  <path strokeLinecap="round" strokeLinejoin="round" d="M5 13l4 4L19 7" />
                </svg>
                <p className="text-sm text-green-800 font-medium">License activated!</p>
              </div>
            </div>

            <p className="text-sm text-stone-600">
              Pick your AI provider and add your API key. You can change this anytime in Settings.
            </p>

            <ProviderPicker
              selectedId={selectedProvider}
              onSelect={handleProviderChange}
              keyStatus={providerKeyStatus}
              compact
            />

            <ApiKeyInput
              providerId={selectedProvider}
              compact
              onSave={handleApiKeySaved}
            />

            <div className="flex gap-3 pt-1">
              {providerKeyStatus[selectedProvider] && (
                <Button
                  variant="primary"
                  size="md"
                  fullWidth
                  onClick={() => setWizardStep('complete')}
                >
                  Continue
                </Button>
              )}
            </div>
          </>
        )}

        {/* ─── Step: Complete ─── */}
        {wizardStep === 'complete' && (
          <>
            <div className="text-center py-4">
              <div className="w-16 h-16 mx-auto mb-4 rounded-full bg-green-100 flex items-center justify-center">
                <svg className="w-8 h-8 text-green-600" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
                  <path strokeLinecap="round" strokeLinejoin="round" d="M5 13l4 4L19 7" />
                </svg>
              </div>
              <p className="text-stone-700 text-sm">
                Paid mode is active. Trial limits have been removed.
              </p>
              <p className="text-stone-500 text-xs mt-1">
                You can manage providers and API keys anytime in Settings.
              </p>
            </div>

            <Button
              variant="primary"
              size="md"
              fullWidth
              onClick={handleComplete}
            >
              Start Using People Partner
            </Button>
          </>
        )}
      </div>
    </Modal>
  );
}

export default UpgradePrompt;
