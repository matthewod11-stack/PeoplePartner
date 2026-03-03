/**
 * SettingsPanel Component
 *
 * Modal for managing app settings including API key, company profile,
 * data location display, telemetry preferences, and intelligence features.
 */

import { useState, useEffect, useCallback } from 'react';
import { Modal } from '../shared/Modal';
import { Button } from '../ui/Button';
import { ApiKeyInput } from './ApiKeyInput';
import { ProviderPicker } from './ProviderPicker';
import { CompanySetup } from '../company/CompanySetup';
import { BackupRestore } from './BackupRestore';
import { PersonaSelector } from './PersonaSelector';
import { SignalsDisclaimerModal } from './SignalsDisclaimerModal';
import { FairnessDisclaimerModal } from './FairnessDisclaimerModal';
import { LicenseKeyInput } from './LicenseKeyInput';
import { ModelSelector } from './ModelSelector';
import { DocumentFolderConfig } from './DocumentFolderConfig';
import {
  getDataPath, getSetting, setSetting,
  getActiveProvider, setActiveProvider, hasProviderApiKey,
} from '../../lib/tauri-commands';
import { useTrial } from '../../contexts/TrialContext';
import { UPGRADE_URL } from '../../lib/constants';
import { PROVIDER_ORDER } from '../../lib/provider-config';

interface SettingsPanelProps {
  /** Whether the panel is open */
  isOpen: boolean;
  /** Called when the panel should close */
  onClose: () => void;
}

export function SettingsPanel({ isOpen, onClose }: SettingsPanelProps) {
  const { isTrialMode, trialStatus, refreshTrialStatus } = useTrial();

  const [dataPath, setDataPath] = useState<string>('');
  const [telemetryEnabled, setTelemetryEnabled] = useState(false);
  const [copyFeedback, setCopyFeedback] = useState(false);

  // V2.4.1: Attention Signals state
  const [signalsEnabled, setSignalsEnabled] = useState(false);
  const [signalsAcknowledged, setSignalsAcknowledged] = useState(false);
  const [showSignalsDisclaimer, setShowSignalsDisclaimer] = useState(false);

  // V2.4.2: Fairness Lens state
  const [fairnessLensEnabled, setFairnessLensEnabled] = useState(false);
  const [fairnessLensAcknowledged, setFairnessLensAcknowledged] = useState(false);
  const [showFairnessDisclaimer, setShowFairnessDisclaimer] = useState(false);

  // Phase E: AI Provider state
  const [activeProviderState, setActiveProviderState] = useState('anthropic');
  const [providerKeyStatus, setProviderKeyStatus] = useState<Record<string, boolean>>({});

  // Load settings on mount
  useEffect(() => {
    if (isOpen) {
      getDataPath()
        .then(setDataPath)
        .catch(() => setDataPath('Unable to determine'));

      getSetting('telemetry_enabled')
        .then((value) => setTelemetryEnabled(value === 'true'))
        .catch(() => setTelemetryEnabled(false));

      // V2.4.1: Load signals settings
      getSetting('signals_enabled')
        .then((value) => setSignalsEnabled(value === 'true'))
        .catch(() => setSignalsEnabled(false));

      getSetting('signals_acknowledged')
        .then((value) => setSignalsAcknowledged(value === 'true'))
        .catch(() => setSignalsAcknowledged(false));

      // V2.4.2: Load fairness lens settings
      getSetting('fairness_lens_enabled')
        .then((value) => setFairnessLensEnabled(value === 'true'))
        .catch(() => setFairnessLensEnabled(false));

      getSetting('fairness_lens_acknowledged')
        .then((value) => setFairnessLensAcknowledged(value === 'true'))
        .catch(() => setFairnessLensAcknowledged(false));

      // Phase E: Load active provider and key status
      getActiveProvider()
        .then(setActiveProviderState)
        .catch(() => setActiveProviderState('anthropic'));

      loadProviderKeyStatus();
    }
  }, [isOpen]);

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
    const previousId = activeProviderState;
    setActiveProviderState(providerId);
    try {
      await setActiveProvider(providerId);
    } catch {
      setActiveProviderState(previousId);
    }
  }, [activeProviderState]);

  const handleCopyPath = useCallback(async () => {
    try {
      await navigator.clipboard.writeText(dataPath);
      setCopyFeedback(true);
      setTimeout(() => setCopyFeedback(false), 2000);
    } catch {
      // Clipboard API may not be available
    }
  }, [dataPath]);

  const handleTelemetryChange = useCallback(async (enabled: boolean) => {
    setTelemetryEnabled(enabled);
    try {
      await setSetting('telemetry_enabled', enabled ? 'true' : 'false');
    } catch {
      // Revert on error
      setTelemetryEnabled(!enabled);
    }
  }, []);

  // V2.4.1: Handle signals toggle
  const handleSignalsToggle = useCallback(async () => {
    if (signalsEnabled) {
      // Disabling - no confirmation needed
      setSignalsEnabled(false);
      try {
        await setSetting('signals_enabled', 'false');
      } catch {
        setSignalsEnabled(true);
      }
    } else {
      // Enabling - check if already acknowledged
      if (signalsAcknowledged) {
        // Already acknowledged, just enable
        setSignalsEnabled(true);
        try {
          await setSetting('signals_enabled', 'true');
        } catch {
          setSignalsEnabled(false);
        }
      } else {
        // Show disclaimer modal
        setShowSignalsDisclaimer(true);
      }
    }
  }, [signalsEnabled, signalsAcknowledged]);

  // V2.4.1: Handle enabling after disclaimer acknowledgment
  const handleSignalsEnable = useCallback(async () => {
    setShowSignalsDisclaimer(false);
    try {
      await setSetting('signals_acknowledged', 'true');
      await setSetting('signals_enabled', 'true');
      setSignalsAcknowledged(true);
      setSignalsEnabled(true);
    } catch {
      // Failed to save
    }
  }, []);

  // V2.4.2: Handle fairness lens toggle
  const handleFairnessLensToggle = useCallback(async () => {
    if (fairnessLensEnabled) {
      // Disabling - no confirmation needed
      setFairnessLensEnabled(false);
      try {
        await setSetting('fairness_lens_enabled', 'false');
      } catch {
        setFairnessLensEnabled(true);
      }
    } else {
      // Enabling - check if already acknowledged
      if (fairnessLensAcknowledged) {
        // Already acknowledged, just enable
        setFairnessLensEnabled(true);
        try {
          await setSetting('fairness_lens_enabled', 'true');
        } catch {
          setFairnessLensEnabled(false);
        }
      } else {
        // Show disclaimer modal
        setShowFairnessDisclaimer(true);
      }
    }
  }, [fairnessLensEnabled, fairnessLensAcknowledged]);

  // V2.4.2: Handle enabling after fairness disclaimer acknowledgment
  const handleFairnessLensEnable = useCallback(async () => {
    setShowFairnessDisclaimer(false);
    try {
      await setSetting('fairness_lens_acknowledged', 'true');
      await setSetting('fairness_lens_enabled', 'true');
      setFairnessLensAcknowledged(true);
      setFairnessLensEnabled(true);
    } catch {
      // Failed to save
    }
  }, []);

  return (
    <>
      <Modal
        isOpen={isOpen}
        onClose={onClose}
        title="Settings"
        maxWidth="max-w-xl"
      >
        <div className="space-y-6">
          {/* Trial Account Section */}
          <section>
            <h3 className="text-sm font-medium text-stone-500 uppercase tracking-wider mb-3">
              Account
            </h3>

            <div className="space-y-3">
              <LicenseKeyInput
                compact
                onSave={refreshTrialStatus}
                onDelete={refreshTrialStatus}
              />

              {trialStatus && (
                <>
                  {isTrialMode ? (
                    <div className="p-4 bg-gradient-to-r from-primary-50 to-amber-50 border border-primary-200 rounded-xl">
                      <div className="flex items-center justify-between">
                        <div>
                          <p className="text-sm font-medium text-stone-700">Free Trial</p>
                          <p className="text-xs text-stone-500 mt-0.5">
                            {trialStatus.messages_used} of {trialStatus.messages_limit} messages used
                          </p>
                        </div>
                        <Button
                          variant="primary"
                          size="sm"
                          onClick={async () => {
                            try {
                              const { open } = await import('@tauri-apps/plugin-shell');
                              await open(UPGRADE_URL);
                            } catch {
                              window.open(UPGRADE_URL, '_blank');
                            }
                          }}
                        >
                          Upgrade &mdash; $99
                        </Button>
                      </div>
                    </div>
                  ) : trialStatus.has_api_key ? (
                    <div className="p-4 bg-green-50 border border-green-200 rounded-xl">
                      <p className="text-sm font-medium text-green-800">Paid Mode Active</p>
                      <p className="text-xs text-green-600 mt-0.5">
                        License and API key are configured. Trial limits are disabled.
                      </p>
                    </div>
                  ) : (
                    <div className="p-4 bg-amber-50 border border-amber-200 rounded-xl">
                      <p className="text-sm font-medium text-amber-800">License Active, API Key Needed</p>
                      <p className="text-xs text-amber-700 mt-0.5">
                        Choose an AI provider and add your API key below to start using paid mode.
                      </p>
                    </div>
                  )}
                </>
              )}
            </div>
          </section>

          {/* AI Provider Section */}
          <section>
            <h3 className="text-sm font-medium text-stone-500 uppercase tracking-wider mb-3">
              AI Provider
            </h3>
            <div className="space-y-3">
              <ProviderPicker
                selectedId={activeProviderState}
                onSelect={handleProviderChange}
                keyStatus={providerKeyStatus}
                compact
              />
              <ModelSelector
                providerId={activeProviderState}
                disabled={isTrialMode}
              />
              <ApiKeyInput
                providerId={activeProviderState}
                compact
                onSave={() => { refreshTrialStatus(); loadProviderKeyStatus(); }}
                onDelete={() => { refreshTrialStatus(); loadProviderKeyStatus(); }}
              />
            </div>
          </section>

          {/* Documents Section (V3.0) */}
          <section>
            <h3 className="text-sm font-medium text-stone-500 uppercase tracking-wider mb-3">
              Documents
            </h3>
            <DocumentFolderConfig />
          </section>

          {/* Company Profile Section */}
          <section>
            <h3 className="text-sm font-medium text-stone-500 uppercase tracking-wider mb-3">
              Company Profile
            </h3>
            <CompanySetup compact />
          </section>

          {/* AI Assistant Style Section (V2.1.3) */}
          <section>
            <h3 className="text-sm font-medium text-stone-500 uppercase tracking-wider mb-3">
              AI Assistant Style
            </h3>
            <PersonaSelector compact />
          </section>

          {/* Intelligence Features Section (V2.4.1) */}
          <section>
            <h3 className="text-sm font-medium text-stone-500 uppercase tracking-wider mb-3">
              Intelligence Features
            </h3>
            <div className="space-y-3">
              {/* Attention Signals Toggle */}
              <div className="flex items-center justify-between gap-4 p-4 bg-stone-50 border border-stone-200 rounded-xl">
                <div className="flex items-center gap-3">
                  <div className="w-8 h-8 flex-shrink-0 flex items-center justify-center rounded-full bg-amber-100">
                    <svg
                      className="w-4 h-4 text-amber-600"
                      fill="none"
                      viewBox="0 0 24 24"
                      stroke="currentColor"
                      strokeWidth={2}
                      aria-hidden="true"
                    >
                      <path
                        strokeLinecap="round"
                        strokeLinejoin="round"
                        d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z"
                      />
                    </svg>
                  </div>
                  <div>
                    <p className="text-sm font-medium text-stone-700">
                      Attention Signals
                    </p>
                    <p className="text-xs text-stone-500">
                      Team-level patterns from tenure, performance, and engagement
                    </p>
                  </div>
                </div>
                <button
                  type="button"
                  role="switch"
                  aria-checked={signalsEnabled}
                  aria-label="Toggle Attention Signals"
                  onClick={handleSignalsToggle}
                  className={`
                    relative inline-flex h-6 w-11 flex-shrink-0 cursor-pointer rounded-full
                    border-2 border-transparent transition-colors duration-200 ease-in-out
                    focus:outline-none focus:ring-2 focus:ring-primary-500 focus:ring-offset-2
                    ${signalsEnabled ? 'bg-primary-500' : 'bg-stone-300'}
                  `}
                >
                  <span
                    aria-hidden="true"
                    className={`
                      pointer-events-none inline-block h-5 w-5 transform rounded-full
                      bg-white shadow ring-0 transition duration-200 ease-in-out
                      ${signalsEnabled ? 'translate-x-5' : 'translate-x-0'}
                    `}
                  />
                </button>
              </div>

              {/* Disclaimer banner when enabled */}
              {signalsEnabled && (
                <div className="p-3 bg-amber-50 border border-amber-200 rounded-lg">
                  <div className="flex gap-2">
                    <svg className="w-4 h-4 text-amber-600 flex-shrink-0 mt-0.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" aria-hidden="true">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                    </svg>
                    <p className="text-xs text-amber-800">
                      Attention signals are heuristic indicators based on aggregate team patterns,
                      not predictions about individuals. Use as conversation starters, not conclusions.
                    </p>
                  </div>
                </div>
              )}

              {/* V2.4.2: Fairness Lens Toggle */}
              <div className="flex items-center justify-between gap-4 p-4 bg-stone-50 border border-stone-200 rounded-xl">
                <div className="flex items-center gap-3">
                  <div className="w-8 h-8 flex-shrink-0 flex items-center justify-center rounded-full bg-teal-100">
                    <svg
                      className="w-4 h-4 text-teal-600"
                      fill="none"
                      viewBox="0 0 24 24"
                      stroke="currentColor"
                      strokeWidth={2}
                      aria-hidden="true"
                    >
                      <path
                        strokeLinecap="round"
                        strokeLinejoin="round"
                        d="M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002 2h2a2 2 0 002-2m0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 01-2 2h-2a2 2 0 01-2-2z"
                      />
                    </svg>
                  </div>
                  <div>
                    <p className="text-sm font-medium text-stone-700">
                      Fairness Lens
                    </p>
                    <p className="text-xs text-stone-500">
                      Demographic representation analysis with privacy guardrails
                    </p>
                  </div>
                </div>
                <button
                  type="button"
                  role="switch"
                  aria-checked={fairnessLensEnabled}
                  aria-label="Toggle Fairness Lens"
                  onClick={handleFairnessLensToggle}
                  className={`
                    relative inline-flex h-6 w-11 flex-shrink-0 cursor-pointer rounded-full
                    border-2 border-transparent transition-colors duration-200 ease-in-out
                    focus:outline-none focus:ring-2 focus:ring-teal-500 focus:ring-offset-2
                    ${fairnessLensEnabled ? 'bg-teal-500' : 'bg-stone-300'}
                  `}
                >
                  <span
                    aria-hidden="true"
                    className={`
                      pointer-events-none inline-block h-5 w-5 transform rounded-full
                      bg-white shadow ring-0 transition duration-200 ease-in-out
                      ${fairnessLensEnabled ? 'translate-x-5' : 'translate-x-0'}
                    `}
                  />
                </button>
              </div>

              {/* Fairness Lens disclaimer banner when enabled */}
              {fairnessLensEnabled && (
                <div className="p-3 bg-teal-50 border border-teal-200 rounded-lg">
                  <div className="flex gap-2">
                    <svg className="w-4 h-4 text-teal-600 flex-shrink-0 mt-0.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" aria-hidden="true">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                    </svg>
                    <p className="text-xs text-teal-800">
                      This analysis reflects historical data patterns and may reveal systemic biases.
                      Groups with fewer than 5 members are suppressed to protect privacy.
                    </p>
                  </div>
                </div>
              )}
            </div>
          </section>

          {/* Data Storage Section */}
          <section>
            <h3 className="text-sm font-medium text-stone-500 uppercase tracking-wider mb-3">
              Data Storage
            </h3>
            <div className="flex items-center justify-between gap-3 p-4 bg-stone-50 border border-stone-200 rounded-xl">
              <div className="flex items-center gap-3 min-w-0">
                <div className="w-8 h-8 flex-shrink-0 flex items-center justify-center rounded-full bg-stone-200">
                  <svg
                    className="w-4 h-4 text-stone-600"
                    fill="none"
                    viewBox="0 0 24 24"
                    stroke="currentColor"
                    strokeWidth={2}
                    aria-hidden="true"
                  >
                    <path
                      strokeLinecap="round"
                      strokeLinejoin="round"
                      d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z"
                    />
                  </svg>
                </div>
                <div className="min-w-0">
                  <p className="text-sm font-medium text-stone-700 truncate" title={dataPath}>
                    {dataPath || 'Loading...'}
                  </p>
                  <p className="text-xs text-stone-500">
                    Database and app data location
                  </p>
                </div>
              </div>
              <button
                type="button"
                onClick={handleCopyPath}
                disabled={!dataPath}
                className="flex-shrink-0 px-3 py-1.5 text-sm text-stone-600 hover:text-stone-800 hover:bg-stone-100 rounded-lg transition-colors disabled:opacity-50"
              >
                {copyFeedback ? (
                  <span className="flex items-center gap-1 text-green-600">
                    <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2} aria-hidden="true">
                      <path strokeLinecap="round" strokeLinejoin="round" d="M5 13l4 4L19 7" />
                    </svg>
                    Copied
                  </span>
                ) : (
                  'Copy'
                )}
              </button>
            </div>
          </section>

          {/* Backup & Restore Section */}
          <section>
            <h3 className="text-sm font-medium text-stone-500 uppercase tracking-wider mb-3">
              Backup & Restore
            </h3>
            <BackupRestore onImportComplete={onClose} />
          </section>

          {/* Privacy Section */}
          <section>
            <h3 className="text-sm font-medium text-stone-500 uppercase tracking-wider mb-3">
              Privacy
            </h3>
            <div className="flex items-center justify-between gap-4 p-4 bg-stone-50 border border-stone-200 rounded-xl">
              <div className="flex items-center gap-3">
                <div className="w-8 h-8 flex-shrink-0 flex items-center justify-center rounded-full bg-stone-200">
                  <svg
                    className="w-4 h-4 text-stone-600"
                    fill="none"
                    viewBox="0 0 24 24"
                    stroke="currentColor"
                    strokeWidth={2}
                    aria-hidden="true"
                  >
                    <path
                      strokeLinecap="round"
                      strokeLinejoin="round"
                      d="M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z"
                    />
                  </svg>
                </div>
                <div>
                  <p className="text-sm font-medium text-stone-700">
                    Anonymous Crash Reports
                  </p>
                  <p className="text-xs text-stone-500">
                    Help improve People Partner by sending anonymous error data
                  </p>
                </div>
              </div>
              <button
                type="button"
                role="switch"
                aria-checked={telemetryEnabled}
                aria-label="Toggle Anonymous Crash Reports"
                onClick={() => handleTelemetryChange(!telemetryEnabled)}
                className={`
                  relative inline-flex h-6 w-11 flex-shrink-0 cursor-pointer rounded-full
                  border-2 border-transparent transition-colors duration-200 ease-in-out
                  focus:outline-none focus:ring-2 focus:ring-primary-500 focus:ring-offset-2
                  ${telemetryEnabled ? 'bg-primary-500' : 'bg-stone-300'}
                `}
              >
                <span
                  aria-hidden="true"
                  className={`
                    pointer-events-none inline-block h-5 w-5 transform rounded-full
                    bg-white shadow ring-0 transition duration-200 ease-in-out
                    ${telemetryEnabled ? 'translate-x-5' : 'translate-x-0'}
                  `}
                />
              </button>
            </div>
          </section>
        </div>
      </Modal>

      {/* V2.4.1: First-use disclaimer modal */}
      <SignalsDisclaimerModal
        isOpen={showSignalsDisclaimer}
        onClose={() => setShowSignalsDisclaimer(false)}
        onEnable={handleSignalsEnable}
      />

      {/* V2.4.2: First-use disclaimer modal for Fairness Lens */}
      <FairnessDisclaimerModal
        isOpen={showFairnessDisclaimer}
        onClose={() => setShowFairnessDisclaimer(false)}
        onEnable={handleFairnessLensEnable}
      />
    </>
  );
}

export default SettingsPanel;
