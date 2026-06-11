// People Partner - Welcome Step (Step 1)
// First impression: logo, value props, get started CTA — and a license-key
// affordance so buyers never exit onboarding in trial mode (#111)

import { useState } from 'react';
import { LicenseKeyInput } from '../../settings/LicenseKeyInput';

interface WelcomeStepProps {
  onContinue: () => void;
  /** A buyer activated their license on the spot — refresh license-aware steps (#111) */
  onLicenseActivated?: () => void;
}

export function WelcomeStep({ onContinue, onLicenseActivated }: WelcomeStepProps) {
  const [showLicense, setShowLicense] = useState(false);
  const [licenseActivated, setLicenseActivated] = useState(false);
  return (
    <div className="flex flex-col items-center text-center">
      {/* Logo */}
      <img src="/logo.png" alt="People Partner" className="w-20 h-20 rounded-2xl shadow-lg shadow-primary-200 mb-6" />

      {/* Headline */}
      <h1 className="text-2xl font-display font-semibold text-stone-800 mb-2">
        Welcome to People Partner
      </h1>
      <p className="text-stone-500 mb-8">
        Your company's HR brain.
      </p>

      {/* Value Props */}
      <div className="w-full space-y-4 mb-8">
        <ValueProp
          icon={
            <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
              <path strokeLinecap="round" strokeLinejoin="round" d="M12 15v2m-6 4h12a2 2 0 002-2v-6a2 2 0 00-2-2H6a2 2 0 00-2 2v6a2 2 0 002 2zm10-10V7a4 4 0 00-8 0v4h8z" />
            </svg>
          }
          title="Completely Private"
          description="Your employee data stays on your Mac. Only your questions go to Claude's AI."
        />
        <ValueProp
          icon={
            <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
              <path strokeLinecap="round" strokeLinejoin="round" d="M17 20h5v-2a3 3 0 00-5.356-1.857M17 20H7m10 0v-2c0-.656-.126-1.283-.356-1.857M7 20H2v-2a3 3 0 015.356-1.857M7 20v-2c0-.656.126-1.283.356-1.857m0 0a5.002 5.002 0 019.288 0M15 7a3 3 0 11-6 0 3 3 0 016 0zm6 3a2 2 0 11-4 0 2 2 0 014 0zM7 10a2 2 0 11-4 0 2 2 0 014 0z" />
            </svg>
          }
          title="Knows Your Team"
          description="Upload employee data and get personalized, contextual HR guidance."
        />
        <ValueProp
          icon={
            <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
              <path strokeLinecap="round" strokeLinejoin="round" d="M9.663 17h4.673M12 3v1m6.364 1.636l-.707.707M21 12h-1M4 12H3m3.343-5.657l-.707-.707m2.828 9.9a5 5 0 117.072 0l-.548.547A3.374 3.374 0 0014 18.469V19a2 2 0 11-4 0v-.531c0-.895-.356-1.754-.988-2.386l-.548-.547z" />
            </svg>
          }
          title="Remembers Conversations"
          description="Cross-conversation memory means compounding value over time."
        />
      </div>

      {/* CTA */}
      <button
        type="button"
        onClick={onContinue}
        className="w-full px-6 py-3 bg-primary-500 hover:bg-primary-600 text-white font-medium rounded-xl transition-all duration-200 shadow-lg shadow-primary-200 hover:shadow-xl hover:brightness-110 active:brightness-95"
      >
        Get Started
      </button>

      {/* Time estimate */}
      <p className="mt-4 text-xs text-stone-500">
        Setup takes about 2 minutes
      </p>

      {/* Already purchased? License key entry (#111) — the purchase email
          sends buyers here with a PP-… key; without this affordance every
          buyer exited onboarding in trial mode. */}
      <div className="mt-6 w-full">
        {licenseActivated ? (
          <div className="p-3 bg-green-50 border border-green-200 rounded-xl text-left">
            <div className="flex items-center gap-2">
              <svg className="w-4 h-4 text-green-600 flex-shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
                <path strokeLinecap="round" strokeLinejoin="round" d="M5 13l4 4L19 7" />
              </svg>
              <p className="text-sm text-green-800 font-medium">
                License activated — paid mode is unlocked.
              </p>
            </div>
            <p className="mt-1 text-xs text-green-700">
              Continue with setup; you&apos;ll add your AI provider key next.
            </p>
          </div>
        ) : showLicense ? (
          <div className="text-left">
            <p className="mb-2 text-sm text-stone-600">
              Paste the license key from your purchase confirmation email.
            </p>
            <LicenseKeyInput
              compact
              onSave={() => {
                setLicenseActivated(true);
                onLicenseActivated?.();
              }}
            />
            <button
              type="button"
              onClick={() => setShowLicense(false)}
              className="mt-2 text-xs text-stone-500 hover:text-stone-700"
            >
              Cancel
            </button>
          </div>
        ) : (
          <button
            type="button"
            onClick={() => setShowLicense(true)}
            className="text-sm text-primary-600 hover:text-primary-700 underline underline-offset-2"
          >
            Already purchased? Enter your license key
          </button>
        )}
      </div>
    </div>
  );
}

// =============================================================================
// Value Prop Component
// =============================================================================

interface ValuePropProps {
  icon: React.ReactNode;
  title: string;
  description: string;
}

function ValueProp({ icon, title, description }: ValuePropProps) {
  return (
    <div className="flex items-start gap-4 p-4 rounded-xl bg-stone-50 text-left">
      <div className="flex-shrink-0 w-10 h-10 rounded-lg bg-primary-100 text-primary-600 flex items-center justify-center">
        {icon}
      </div>
      <div>
        <h3 className="font-medium text-stone-800">{title}</h3>
        <p className="text-sm text-stone-500">{description}</p>
      </div>
    </div>
  );
}

export default WelcomeStep;
