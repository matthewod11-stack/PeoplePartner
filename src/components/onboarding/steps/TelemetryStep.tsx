// People Partner - Telemetry Step (Step 6)
// Optional: anonymous crash reports to help improve the product

import { useState, useCallback } from 'react';
import { setSetting } from '../../../lib/tauri-commands';
import { SETTINGS_KEYS } from '../OnboardingContext';

interface TelemetryStepProps {
  onContinue: () => void;
}

export function TelemetryStep({ onContinue }: TelemetryStepProps) {
  const [enabled, setEnabled] = useState(false);
  const [isSaving, setIsSaving] = useState(false);

  const handleToggle = useCallback(() => {
    setEnabled((prev) => !prev);
  }, []);

  const handleContinue = useCallback(async (choice: boolean) => {
    setIsSaving(true);
    try {
      await setSetting(SETTINGS_KEYS.TELEMETRY_ENABLED, String(choice));
      onContinue();
    } catch (err) {
      console.error('[Onboarding] Failed to save telemetry preference:', err);
      // Continue anyway
      onContinue();
    }
  }, [onContinue]);

  return (
    <div className="w-full">
      {/* Icon */}
      <div className="flex justify-center mb-6">
        <div className="w-14 h-14 rounded-xl bg-primary-100 flex items-center justify-center">
          <svg className="w-7 h-7 text-primary-600" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
            <path strokeLinecap="round" strokeLinejoin="round" d="M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002 2h2a2 2 0 002-2m0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 01-2 2h-2a2 2 0 01-2-2z" />
          </svg>
        </div>
      </div>

      {/* Toggle */}
      <div className="bg-stone-50 rounded-xl p-4 mb-6">
        <div className="flex items-center justify-between">
          <div>
            <h4 className="font-medium text-stone-800">Send anonymous crash reports</h4>
            <p className="text-sm text-stone-500 mt-1">Helps us fix bugs faster</p>
          </div>
          <button
            type="button"
            onClick={handleToggle}
            disabled={isSaving}
            className={`
              relative w-12 h-7 rounded-full transition-colors duration-200
              ${enabled ? 'bg-primary-500' : 'bg-stone-300'}
              ${isSaving ? 'opacity-50 cursor-not-allowed' : ''}
            `}
            role="switch"
            aria-checked={enabled}
          >
            <span
              className={`
                absolute top-1 left-1 w-5 h-5 rounded-full bg-white shadow-sm
                transition-transform duration-200
                ${enabled ? 'translate-x-5' : 'translate-x-0'}
              `}
            />
          </button>
        </div>
      </div>

      {/* Privacy assurance */}
      <div className="space-y-3 mb-8">
        <div className="flex items-start gap-3">
          <svg className="w-5 h-5 text-green-500 mt-0.5 flex-shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
            <path strokeLinecap="round" strokeLinejoin="round" d="M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z" />
          </svg>
          <p className="text-sm text-stone-600">
            <strong className="text-stone-800">No employee data</strong> — only crash reports and app errors
          </p>
        </div>
        <div className="flex items-start gap-3">
          <svg className="w-5 h-5 text-green-500 mt-0.5 flex-shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
            <path strokeLinecap="round" strokeLinejoin="round" d="M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z" />
          </svg>
          <p className="text-sm text-stone-600">
            <strong className="text-stone-800">No chat content</strong> — your conversations stay private
          </p>
        </div>
        <div className="flex items-start gap-3">
          <svg className="w-5 h-5 text-green-500 mt-0.5 flex-shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
            <path strokeLinecap="round" strokeLinejoin="round" d="M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z" />
          </svg>
          <p className="text-sm text-stone-600">
            <strong className="text-stone-800">Change anytime</strong> — toggle in Settings later
          </p>
        </div>
      </div>

      {/* Action buttons */}
      <div className="flex gap-3">
        <button
          type="button"
          onClick={() => handleContinue(false)}
          disabled={isSaving}
          className="flex-1 px-4 py-3 text-stone-600 hover:text-stone-800 hover:bg-stone-100 font-medium rounded-xl transition-colors"
        >
          No thanks
        </button>
        <button
          type="button"
          onClick={() => handleContinue(true)}
          disabled={isSaving}
          className="flex-1 px-4 py-3 bg-primary-500 hover:bg-primary-600 text-white font-medium rounded-xl transition-all duration-200"
        >
          {enabled ? 'Enable & Continue' : 'Continue'}
        </button>
      </div>
    </div>
  );
}

export default TelemetryStep;
