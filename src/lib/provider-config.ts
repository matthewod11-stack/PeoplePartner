// People Partner - Provider Display Metadata
// UI-only constants for provider branding, setup guides, and console URLs

export interface ProviderMeta {
  displayName: string;
  modelName: string;
  description: string;
  consoleUrl: string;
  keysUrl: string;
  keyPrefixHint: string;
  iconColor: string;
  bgColor: string;
  borderColor: string;
  selectedBg: string;
  selectedBorder: string;
  setupSteps: {
    signup: string;
    billing: string;
    createKey: string;
  };
}

export const PROVIDER_META: Record<string, ProviderMeta> = {
  anthropic: {
    displayName: 'Anthropic',
    modelName: 'Claude',
    description: 'Claude by Anthropic — thoughtful, nuanced HR guidance',
    consoleUrl: 'https://console.anthropic.com',
    keysUrl: 'https://console.anthropic.com/settings/keys',
    keyPrefixHint: 'sk-ant-...',
    iconColor: 'text-amber-600',
    bgColor: 'bg-amber-50',
    borderColor: 'border-amber-200',
    selectedBg: 'bg-amber-50',
    selectedBorder: 'border-amber-400',
    setupSteps: {
      signup: 'Visit console.anthropic.com and create an account (or sign in).',
      billing: 'Go to Settings \u2192 Billing and add a payment method. You only pay for what you use.',
      createKey: 'Go to Settings \u2192 API Keys, click "Create Key", and name it "People Partner".',
    },
  },
  openai: {
    displayName: 'OpenAI',
    modelName: 'GPT-4o',
    description: 'GPT-4o by OpenAI — fast, versatile AI assistant',
    consoleUrl: 'https://platform.openai.com',
    keysUrl: 'https://platform.openai.com/api-keys',
    keyPrefixHint: 'sk-...',
    iconColor: 'text-green-600',
    bgColor: 'bg-green-50',
    borderColor: 'border-green-200',
    selectedBg: 'bg-green-50',
    selectedBorder: 'border-green-400',
    setupSteps: {
      signup: 'Visit platform.openai.com and create an account (or sign in).',
      billing: 'Go to Settings \u2192 Billing and add a payment method. You only pay for what you use.',
      createKey: 'Go to API Keys, click "Create new secret key", and name it "People Partner".',
    },
  },
  gemini: {
    displayName: 'Google Gemini',
    modelName: 'Gemini Pro',
    description: 'Gemini by Google — powerful multimodal AI',
    consoleUrl: 'https://aistudio.google.com',
    keysUrl: 'https://aistudio.google.com/apikey',
    keyPrefixHint: 'AIzaSy...',
    iconColor: 'text-blue-600',
    bgColor: 'bg-blue-50',
    borderColor: 'border-blue-200',
    selectedBg: 'bg-blue-50',
    selectedBorder: 'border-blue-400',
    setupSteps: {
      signup: 'Visit aistudio.google.com and sign in with your Google account.',
      billing: 'Gemini offers a free tier. For higher limits, enable billing in Google Cloud Console.',
      createKey: 'Click "Create API Key" and select or create a Google Cloud project.',
    },
  },
};

/** Ordered list of provider IDs for consistent display */
export const PROVIDER_ORDER = ['anthropic', 'openai', 'gemini'] as const;
