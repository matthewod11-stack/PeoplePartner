// People Partner - First Prompt Step (Step 7)
// Celebration + contextual prompt suggestions to get started

import { PromptSuggestions, type PromptSuggestion } from '../../chat/PromptSuggestions';

/**
 * Static suggestions for onboarding completion
 * Note: We use static suggestions here because the EmployeeProvider
 * isn't mounted during onboarding. These are generic "getting started" prompts.
 */
const ONBOARDING_SUGGESTIONS: PromptSuggestion[] = [
  {
    text: 'Who has an anniversary this month?',
    icon: '🎂',
    category: 'people',
  },
  {
    text: "What's our team eNPS?",
    icon: '📊',
    category: 'analytics',
  },
  {
    text: 'Who are our top performers?',
    icon: '⭐',
    category: 'analytics',
  },
  {
    text: 'Help me draft a performance review',
    icon: '✍️',
    category: 'general',
  },
];

interface FirstPromptStepProps {
  onStart: (prompt?: string) => void;
  initialPrompt?: string;
}

export function FirstPromptStep({ onStart }: FirstPromptStepProps) {
  const suggestions = ONBOARDING_SUGGESTIONS;

  const handleSuggestionClick = (prompt: string) => {
    // Immediately start with this prompt
    onStart(prompt);
  };

  const handleStartWithoutPrompt = () => {
    onStart();
  };

  return (
    <div className="w-full text-center">
      {/* Celebration icon */}
      <div className="flex justify-center mb-6">
        <div className="w-20 h-20 rounded-2xl bg-gradient-to-br from-green-400 to-green-600 flex items-center justify-center shadow-lg shadow-green-200">
          <svg className="w-10 h-10 text-white" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
            <path strokeLinecap="round" strokeLinejoin="round" d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z" />
          </svg>
        </div>
      </div>

      {/* Headline */}
      <h1 className="text-2xl font-display font-semibold text-stone-800 mb-2">
        You're all set!
      </h1>
      <p className="text-stone-500 mb-8">
        People Partner is ready to help. Try asking about your team.
      </p>

      {/* Prompt Suggestions */}
      <div className="mb-6">
        <p className="text-sm text-stone-500 mb-3">Try one of these to get started:</p>
        <PromptSuggestions
          variant="welcome"
          suggestions={suggestions}
          onSelect={handleSuggestionClick}
        />
      </div>

      {/* Or start chatting */}
      <div className="relative my-6">
        <div className="absolute inset-0 flex items-center">
          <div className="w-full border-t border-stone-200" />
        </div>
        <div className="relative flex justify-center">
          <span className="bg-white px-4 text-sm text-stone-500">or</span>
        </div>
      </div>

      <button
        type="button"
        onClick={handleStartWithoutPrompt}
        className="w-full px-6 py-3 bg-primary-500 hover:bg-primary-600 text-white font-medium rounded-xl transition-all duration-200 shadow-lg shadow-primary-200 hover:shadow-xl hover:brightness-110 active:brightness-95"
      >
        Start Chatting
      </button>

      {/* Meet Alex */}
      <div className="mt-8 p-4 bg-stone-50 rounded-xl text-left">
        <div className="flex items-start gap-3">
          <div className="w-10 h-10 rounded-full bg-primary-100 flex items-center justify-center flex-shrink-0">
            <span className="text-lg">👋</span>
          </div>
          <div>
            <p className="text-sm text-stone-600">
              <strong className="text-stone-800">Meet Alex</strong> — your AI HR advisor with 20+ years of
              experience. Alex knows your company, your team, and remembers your past conversations.
            </p>
          </div>
        </div>
      </div>
    </div>
  );
}

export default FirstPromptStep;
