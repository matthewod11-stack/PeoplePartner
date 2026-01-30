/**
 * SettingsPanel Component
 *
 * Modal for managing app settings including API key, company profile,
 * data location display, telemetry preferences, and intelligence features.
 */

import { useState, useEffect, useCallback } from 'react';
import { Modal } from '../shared/Modal';
import { ApiKeyInput } from './ApiKeyInput';
import { CompanySetup } from '../company/CompanySetup';
import { BackupRestore } from './BackupRestore';
import { PersonaSelector } from './PersonaSelector';
import { SignalsDisclaimerModal } from './SignalsDisclaimerModal';
import { getDataPath, getSetting, setSetting } from '../../lib/tauri-commands';

interface SettingsPanelProps {
  /** Whether the panel is open */
  isOpen: boolean;
  /** Called when the panel should close */
  onClose: () => void;
}

export function SettingsPanel({ isOpen, onClose }: SettingsPanelProps) {
  const [dataPath, setDataPath] = useState<string>('');
  const [telemetryEnabled, setTelemetryEnabled] = useState(false);
  const [copyFeedback, setCopyFeedback] = useState(false);

  // V2.4.1: Attention Signals state
  const [signalsEnabled, setSignalsEnabled] = useState(false);
  const [signalsAcknowledged, setSignalsAcknowledged] = useState(false);
  const [showSignalsDisclaimer, setShowSignalsDisclaimer] = useState(false);

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
    }
  }, [isOpen]);

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

  return (
    <>
      <Modal
        isOpen={isOpen}
        onClose={onClose}
        title="Settings"
        maxWidth="max-w-xl"
      >
        <div className="space-y-6">
          {/* API Connection Section */}
          <section>
            <h3 className="text-sm font-medium text-stone-500 uppercase tracking-wider mb-3">
              API Connection
            </h3>
            <ApiKeyInput compact />
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
                    <svg className="w-4 h-4 text-amber-600 flex-shrink-0 mt-0.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                    </svg>
                    <p className="text-xs text-amber-800">
                      Attention signals are heuristic indicators based on aggregate team patterns,
                      not predictions about individuals. Use as conversation starters, not conclusions.
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
                    <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
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
                    Help improve HR Command Center by sending anonymous error data
                  </p>
                </div>
              </div>
              <button
                type="button"
                role="switch"
                aria-checked={telemetryEnabled}
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
    </>
  );
}

export default SettingsPanel;
