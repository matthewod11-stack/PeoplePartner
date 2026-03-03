// People Partner - AI Provider Setup Step (Step 2)
// Two-phase flow: 1) Pick provider, 2) Enter API key with provider-specific guide

import { useEffect, useState } from 'react';
import { ApiKeyInput } from '../../settings/ApiKeyInput';
import { ProviderPicker } from '../../settings/ProviderPicker';
import { getActiveProvider, setActiveProvider, hasAnyProviderApiKey } from '../../../lib/tauri-commands';
import { PROVIDER_META } from '../../../lib/provider-config';
import { useTrial } from '../../../contexts/TrialContext';

interface ApiKeyStepProps {
  onComplete: () => void;
  onValidChange: (valid: boolean) => void;
}

// Collapsible section component
function CollapsibleSection({
  title,
  isOpen,
  onToggle,
  children,
}: {
  title: string;
  isOpen: boolean;
  onToggle: () => void;
  children: React.ReactNode;
}) {
  return (
    <div className="mb-4">
      <button
        type="button"
        onClick={onToggle}
        className="flex items-center gap-2 text-sm font-medium text-stone-600 hover:text-stone-800 transition-colors"
      >
        <svg
          className={`w-4 h-4 transition-transform duration-200 ${isOpen ? 'rotate-90' : ''}`}
          fill="none"
          viewBox="0 0 24 24"
          stroke="currentColor"
          strokeWidth={2}
        >
          <path strokeLinecap="round" strokeLinejoin="round" d="M9 5l7 7-7 7" />
        </svg>
        {title}
      </button>
      {isOpen && (
        <div className="mt-3 ml-6 animate-fadeIn">
          {children}
        </div>
      )}
    </div>
  );
}

// FAQ Item component
function FAQItem({ question, answer }: { question: string; answer: string }) {
  const [isOpen, setIsOpen] = useState(false);

  return (
    <div className="border-b border-stone-100 last:border-0">
      <button
        type="button"
        onClick={() => setIsOpen(!isOpen)}
        className="w-full py-2 flex items-start gap-2 text-left text-sm text-stone-700 hover:text-stone-900"
      >
        <svg
          className={`w-4 h-4 mt-0.5 flex-shrink-0 transition-transform duration-200 ${isOpen ? 'rotate-90' : ''}`}
          fill="none"
          viewBox="0 0 24 24"
          stroke="currentColor"
          strokeWidth={2}
        >
          <path strokeLinecap="round" strokeLinejoin="round" d="M9 5l7 7-7 7" />
        </svg>
        <span className="font-medium">{question}</span>
      </button>
      {isOpen && (
        <p className="pb-3 pl-6 text-sm text-stone-600 leading-relaxed">
          {answer}
        </p>
      )}
    </div>
  );
}

export function ApiKeyStep({ onComplete, onValidChange }: ApiKeyStepProps) {
  const { isTrialMode } = useTrial();
  const [phase, setPhase] = useState<'select' | 'configure'>('select');
  const [selectedProvider, setSelectedProvider] = useState('anthropic');
  const [hasKey, setHasKey] = useState(false);
  const [showWhatIs, setShowWhatIs] = useState(false);
  const [showSteps, setShowSteps] = useState(true);
  const [showTroubleshooting, setShowTroubleshooting] = useState(false);

  const meta = PROVIDER_META[selectedProvider];

  // Load active provider and check if key exists on mount
  useEffect(() => {
    getActiveProvider().then((id) => {
      setSelectedProvider(id);
    }).catch(() => {
      // Default to anthropic
    });

    hasAnyProviderApiKey().then((exists) => {
      setHasKey(exists);
      onValidChange(exists);
      if (exists) {
        setShowSteps(false);
      }
    }).catch(() => {
      // Ignore errors
    });
  }, [onValidChange]);

  const handleProviderSelect = (id: string) => {
    setSelectedProvider(id);
  };

  const handleContinueToConfig = async () => {
    // Persist the provider choice
    try {
      await setActiveProvider(selectedProvider);
    } catch {
      // Non-fatal — provider will default to anthropic
    }
    setPhase('configure');
  };

  const handleSave = () => {
    setHasKey(true);
    onValidChange(true);
    onComplete();
  };

  // Phase 1: Provider selection
  if (phase === 'select') {
    return (
      <div className="w-full overflow-y-auto max-h-[calc(100vh-320px)]">
        {isTrialMode && (
          <div className="mb-4 p-3 bg-stone-50 rounded-lg border border-stone-200">
            <p className="text-xs text-stone-600">
              Trial uses Claude (Anthropic). Choose your preferred provider for when you upgrade to paid mode.
            </p>
          </div>
        )}

        <ProviderPicker
          selectedId={selectedProvider}
          onSelect={handleProviderSelect}
        />

        <div className="mt-6 flex justify-center">
          <button
            type="button"
            onClick={handleContinueToConfig}
            className="px-6 py-3 bg-primary-500 hover:bg-primary-600 text-white font-medium rounded-xl transition-all duration-200 shadow-sm hover:shadow-md"
          >
            Continue
          </button>
        </div>
      </div>
    );
  }

  // Phase 2: API key entry with provider-specific guide
  return (
    <div className="w-full overflow-y-auto max-h-[calc(100vh-320px)]">
      {/* Change provider link */}
      <button
        type="button"
        onClick={() => setPhase('select')}
        className="mb-4 flex items-center gap-1 text-sm text-primary-600 hover:text-primary-700 transition-colors"
      >
        <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
          <path strokeLinecap="round" strokeLinejoin="round" d="M15 19l-7-7 7-7" />
        </svg>
        Change provider
      </button>

      {/* What is an API key? - Collapsible explainer */}
      <CollapsibleSection
        title="What is an API key?"
        isOpen={showWhatIs}
        onToggle={() => setShowWhatIs(!showWhatIs)}
      >
        <div className="bg-stone-50 rounded-xl p-4 text-sm text-stone-600 leading-relaxed">
          <p className="mb-3">
            An API key is like a <strong className="text-stone-800">password</strong> that lets
            People Partner talk to {meta?.displayName ?? 'your AI provider'}.
          </p>
          <p className="mb-3">
            Think of it like a library card — it identifies you and lets you access the service.
          </p>
          <p>
            Your key is stored <strong className="text-stone-800">securely on your Mac</strong> and
            is only sent to {meta?.displayName ?? 'the provider'} when you ask questions.
          </p>
        </div>
      </CollapsibleSection>

      {/* Step-by-step guide */}
      <CollapsibleSection
        title={`Setting up your ${meta?.displayName ?? ''} API key`}
        isOpen={showSteps}
        onToggle={() => setShowSteps(!showSteps)}
      >
        <div className="bg-stone-50 rounded-xl p-4 text-sm text-stone-600 leading-relaxed">
          <ol className="space-y-4">
            <li className="flex gap-3">
              <span className="flex-shrink-0 w-6 h-6 rounded-full bg-primary-100 text-primary-700 text-xs font-semibold flex items-center justify-center">
                1
              </span>
              <div>
                <p className="font-medium text-stone-800">Create an account</p>
                <p className="mt-1">
                  {meta?.setupSteps.signup}{' '}
                  <a
                    href={meta?.consoleUrl}
                    target="_blank"
                    rel="noopener noreferrer"
                    className="text-primary-600 hover:text-primary-700 underline"
                  >
                    Open {meta?.displayName} Console
                  </a>
                </p>
              </div>
            </li>
            <li className="flex gap-3">
              <span className="flex-shrink-0 w-6 h-6 rounded-full bg-primary-100 text-primary-700 text-xs font-semibold flex items-center justify-center">
                2
              </span>
              <div>
                <p className="font-medium text-stone-800">Set up billing</p>
                <p className="mt-1">
                  {meta?.setupSteps.billing}
                </p>
              </div>
            </li>
            <li className="flex gap-3">
              <span className="flex-shrink-0 w-6 h-6 rounded-full bg-primary-100 text-primary-700 text-xs font-semibold flex items-center justify-center">
                3
              </span>
              <div>
                <p className="font-medium text-stone-800">Create your API key</p>
                <p className="mt-1">
                  {meta?.setupSteps.createKey}{' '}
                  <a
                    href={meta?.keysUrl}
                    target="_blank"
                    rel="noopener noreferrer"
                    className="text-primary-600 hover:text-primary-700 underline"
                  >
                    Go to API Keys
                  </a>
                </p>
                <p className="mt-1 text-amber-600 font-medium">
                  Copy the key immediately — it won't be shown again!
                </p>
              </div>
            </li>
            <li className="flex gap-3">
              <span className="flex-shrink-0 w-6 h-6 rounded-full bg-primary-100 text-primary-700 text-xs font-semibold flex items-center justify-center">
                4
              </span>
              <div>
                <p className="font-medium text-stone-800">Paste it below</p>
                <p className="mt-1">
                  Paste the key in the field below and click "Save Key".
                </p>
              </div>
            </li>
          </ol>
        </div>
      </CollapsibleSection>

      {/* API Key Input */}
      <div className="my-6">
        <ApiKeyInput
          providerId={selectedProvider}
          onSave={handleSave}
          compact={false}
        />
      </div>

      {/* Cost information */}
      <div className="mb-6 p-3 bg-stone-50 rounded-lg border border-stone-100">
        <div className="flex items-start gap-2">
          <svg className="w-4 h-4 mt-0.5 text-stone-500 flex-shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
            <path strokeLinecap="round" strokeLinejoin="round" d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
          </svg>
          <div className="text-xs text-stone-600">
            <p>
              <strong className="text-stone-700">About costs:</strong> Most users spend{' '}
              <strong className="text-stone-700">$5–15/month</strong> — roughly the price of a coffee or two.
            </p>
            <p className="mt-1 text-stone-500">
              You only pay for what you use. No monthly minimums or commitments.
            </p>
            <p className="mt-1 text-primary-600 font-medium">
              If you purchased a license, enter it in Settings to unlock paid mode.
            </p>
          </div>
        </div>
      </div>

      {/* Troubleshooting - Collapsible FAQ */}
      <CollapsibleSection
        title="Having trouble?"
        isOpen={showTroubleshooting}
        onToggle={() => setShowTroubleshooting(!showTroubleshooting)}
      >
        <div className="bg-stone-50 rounded-xl p-4">
          <FAQItem
            question={`My key doesn't start with ${meta?.keyPrefixHint ?? 'the expected prefix'}`}
            answer={`${meta?.displayName ?? 'This provider'}'s API keys start with "${meta?.keyPrefixHint ?? ''}". If yours starts with a different prefix, make sure you're copying a key from the right provider.`}
          />
          <FAQItem
            question="I copied the key but it says invalid"
            answer="API keys are only shown once when created. If you didn't copy the full key, you'll need to create a new one. Make sure to copy it completely!"
          />
          <FAQItem
            question="I need to add billing first"
            answer="Before creating an API key, you may need to add a payment method. Check the provider's billing settings and add a credit card if required."
          />
          <FAQItem
            question="The key saved but chat doesn't work"
            answer="First, check your internet connection. If that's fine, verify that your account has billing set up and isn't over any spending limits. You can also try removing and re-adding the key."
          />
        </div>
      </CollapsibleSection>

      {/* Already configured - Continue button */}
      {hasKey && (
        <div className="mt-6 text-center">
          <button
            type="button"
            onClick={onComplete}
            className="px-6 py-3 bg-primary-500 hover:bg-primary-600 text-white font-medium rounded-xl transition-all duration-200"
          >
            Continue
          </button>
        </div>
      )}
    </div>
  );
}

export default ApiKeyStep;
