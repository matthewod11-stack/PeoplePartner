// HR Command Center - API Key Step (Step 2)
// Enhanced with beginner-friendly guide for non-technical HR users

import { useEffect, useState } from 'react';
import { ApiKeyInput } from '../../settings/ApiKeyInput';
import { hasApiKey } from '../../../lib/tauri-commands';

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
  const [hasKey, setHasKey] = useState(false);
  const [showWhatIs, setShowWhatIs] = useState(false);
  const [showSteps, setShowSteps] = useState(true); // Open by default for new users
  const [showTroubleshooting, setShowTroubleshooting] = useState(false);

  // Check if key already exists on mount
  useEffect(() => {
    hasApiKey().then((exists) => {
      setHasKey(exists);
      onValidChange(exists);
      // If user already has a key, collapse the steps guide
      if (exists) {
        setShowSteps(false);
      }
    }).catch(() => {
      // Ignore errors
    });
  }, [onValidChange]);

  const handleSave = () => {
    setHasKey(true);
    onValidChange(true);
    // Auto-advance to next step
    onComplete();
  };

  return (
    <div className="w-full overflow-y-auto max-h-[calc(100vh-320px)]">
      {/* What is an API key? - Collapsible explainer */}
      <CollapsibleSection
        title="What is an API key?"
        isOpen={showWhatIs}
        onToggle={() => setShowWhatIs(!showWhatIs)}
      >
        <div className="bg-stone-50 rounded-xl p-4 text-sm text-stone-600 leading-relaxed">
          <p className="mb-3">
            An API key is like a <strong className="text-stone-800">password</strong> that lets
            HR Command Center talk to Claude, the AI assistant.
          </p>
          <p className="mb-3">
            Think of it like a library card — it identifies you and lets you access the service.
          </p>
          <p>
            Your key is stored <strong className="text-stone-800">securely on your Mac</strong> and
            is only sent to Anthropic (Claude's creator) when you ask questions.
          </p>
        </div>
      </CollapsibleSection>

      {/* Step-by-step guide - Open by default for new users */}
      <CollapsibleSection
        title="Setting up your API key"
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
                <p className="font-medium text-stone-800">Create an Anthropic account</p>
                <p className="mt-1">
                  Visit{' '}
                  <a
                    href="https://console.anthropic.com"
                    target="_blank"
                    rel="noopener noreferrer"
                    className="text-primary-600 hover:text-primary-700 underline"
                  >
                    console.anthropic.com
                  </a>
                  {' '}and sign up (or sign in if you have an account).
                </p>
              </div>
            </li>
            <li className="flex gap-3">
              <span className="flex-shrink-0 w-6 h-6 rounded-full bg-primary-100 text-primary-700 text-xs font-semibold flex items-center justify-center">
                2
              </span>
              <div>
                <p className="font-medium text-stone-800">Add billing information</p>
                <p className="mt-1">
                  Go to <strong>Settings → Billing</strong> and add a payment method.
                  <span className="block mt-1 text-stone-500">
                    Don't worry — you only pay for what you use.
                  </span>
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
                  Go to{' '}
                  <a
                    href="https://console.anthropic.com/settings/keys"
                    target="_blank"
                    rel="noopener noreferrer"
                    className="text-primary-600 hover:text-primary-700 underline"
                  >
                    Settings → API Keys
                  </a>
                  , click <strong>"Create Key"</strong>, and give it a name like "HR Command Center".
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
          onSave={handleSave}
          compact={false}
        />
      </div>

      {/* Cost information - Reassuring, not alarming */}
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
            question="My key doesn't start with sk-ant-"
            answer="Anthropic API keys always start with 'sk-ant-'. If yours starts with just 'sk-', that's an OpenAI key — you need one from Anthropic instead."
          />
          <FAQItem
            question="I copied the key but it says invalid"
            answer="API keys are only shown once when created. If you didn't copy the full key, you'll need to create a new one in the Anthropic console. Make sure to copy it completely!"
          />
          <FAQItem
            question="I need to add billing first"
            answer="Before creating an API key, you need to add a payment method. Go to Settings → Billing in the Anthropic console and add a credit card. You won't be charged until you actually use the service."
          />
          <FAQItem
            question="The key saved but chat doesn't work"
            answer="First, check your internet connection. If that's fine, verify that your Anthropic account has billing set up and isn't over any spending limits. You can also try removing and re-adding the key."
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
