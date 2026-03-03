// People Partner - Disclaimer Step (Step 5)
// Legal acknowledgment: AI is informational, not legal advice

import { useState, useCallback } from 'react';

interface DisclaimerStepProps {
  onAccept: () => void;
  onValidChange: (valid: boolean) => void;
}

export function DisclaimerStep({ onAccept, onValidChange }: DisclaimerStepProps) {
  const [accepted, setAccepted] = useState(false);

  const handleCheckboxChange = useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
    const checked = e.target.checked;
    setAccepted(checked);
    onValidChange(checked);
  }, [onValidChange]);

  const handleAccept = () => {
    if (accepted) {
      onAccept();
    }
  };

  return (
    <div className="w-full">
      {/* Info icon (softer than warning) */}
      <div className="flex justify-center mb-6">
        <div className="w-14 h-14 rounded-xl bg-teal-100 flex items-center justify-center">
          <svg className="w-7 h-7 text-teal-600" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
            <path strokeLinecap="round" strokeLinejoin="round" d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
          </svg>
        </div>
      </div>

      {/* Disclaimer content */}
      <div className="bg-stone-50 rounded-xl p-4 mb-6 max-h-64 overflow-y-auto text-sm text-stone-600 leading-relaxed">
        <h4 className="font-semibold text-stone-800 mb-3">Good to know</h4>

        <p className="mb-3">
          People Partner uses AI to provide HR guidance and assistance. While designed to be helpful,
          <strong className="text-stone-800"> this tool does not provide legal, tax, or professional advice</strong>.
        </p>

        <p className="mb-3">
          The information provided is for general informational purposes only and should not be relied upon
          as a substitute for consultation with qualified legal, HR, or other professional advisors.
        </p>

        <h5 className="font-medium text-stone-800 mt-4 mb-2">You acknowledge that:</h5>
        <ul className="list-disc pl-5 space-y-2">
          <li>
            AI responses may contain errors, omissions, or outdated information
          </li>
          <li>
            Employment laws vary by jurisdiction and change frequently
          </li>
          <li>
            Decisions involving employees should be reviewed by qualified professionals
          </li>
          <li>
            You are responsible for verifying any information before acting on it
          </li>
          <li>
            This tool does not create an attorney-client or professional-client relationship
          </li>
        </ul>

        <h5 className="font-medium text-stone-800 mt-4 mb-2">Data handling:</h5>
        <ul className="list-disc pl-5 space-y-2">
          <li>
            Your employee data is stored locally on your Mac and is never sent to our servers
          </li>
          <li>
            Conversations are sent to Anthropic's Claude API using your own API key
          </li>
          <li>
            Financial PII (SSN, credit cards, bank accounts) is automatically redacted before sending
          </li>
        </ul>
      </div>

      {/* Checkbox */}
      <label className="flex items-start gap-3 mb-6 cursor-pointer group">
        <input
          type="checkbox"
          checked={accepted}
          onChange={handleCheckboxChange}
          className="mt-0.5 w-5 h-5 text-primary-500 border-stone-300 rounded focus:ring-primary-500 cursor-pointer"
        />
        <span className="text-sm text-stone-700 group-hover:text-stone-900">
          I understand that People Partner provides <strong>informational guidance only</strong> and
          is not a substitute for professional legal or HR advice.
        </span>
      </label>

      {/* Continue button */}
      <button
        type="button"
        onClick={handleAccept}
        disabled={!accepted}
        className={`
          w-full px-6 py-3 font-medium rounded-xl transition-all duration-200
          ${
            accepted
              ? 'bg-primary-500 hover:bg-primary-600 text-white'
              : 'bg-stone-200 text-stone-400 cursor-not-allowed'
          }
        `}
      >
        I Understand, Continue
      </button>
    </div>
  );
}

export default DisclaimerStep;
