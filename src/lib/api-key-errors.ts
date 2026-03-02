// API Key Error Hints
// Maps validation failures to user-friendly guidance for non-technical users

export type ApiKeyErrorType =
  | 'empty'
  | 'wrong_prefix'
  | 'missing_prefix'
  | 'too_short'
  | 'storage_failure';

/**
 * Returns a user-friendly hint based on the API key format.
 * Returns null if the key appears valid or is empty.
 *
 * When providerId is set, gives provider-specific guidance.
 * When undefined, uses legacy Anthropic-only behavior.
 */
export function getApiKeyErrorHint(key: string, providerId?: string): string | null {
  if (!key) {
    return null;
  }

  if (providerId === 'openai') {
    if (key.startsWith('sk-ant-')) {
      return "This looks like an Anthropic key (starts with 'sk-ant-'). You need an OpenAI key — it starts with 'sk-'";
    }
    if (key.startsWith('AIzaSy')) {
      return "This looks like a Google Gemini key. You need an OpenAI key — it starts with 'sk-'";
    }
    if (!key.startsWith('sk-')) {
      return "Make sure you copied the full key — OpenAI keys start with 'sk-'";
    }
    return null;
  }

  if (providerId === 'gemini') {
    if (key.startsWith('sk-')) {
      return "This looks like an OpenAI or Anthropic key. You need a Google Gemini key — it starts with 'AIzaSy'";
    }
    if (!key.startsWith('AIzaSy')) {
      return "Make sure you copied the full key — Gemini keys start with 'AIzaSy'";
    }
    return null;
  }

  // Anthropic (default / legacy)
  if (key.startsWith('sk-') && !key.startsWith('sk-ant-')) {
    return "This looks like an OpenAI key (starts with 'sk-'). You need an Anthropic key — it starts with 'sk-ant-'";
  }

  if (key.startsWith('AIzaSy')) {
    return "This looks like a Google Gemini key. You need an Anthropic key — it starts with 'sk-ant-'";
  }

  if (!key.startsWith('sk-ant-')) {
    return "Make sure you copied the full key — it should start with 'sk-ant-'";
  }

  if (key.length < 40) {
    return 'This key seems incomplete. Anthropic keys are usually longer. Did you copy the whole thing?';
  }

  return null;
}

/**
 * Returns a user-friendly message for storage/backend errors.
 */
export function getStorageErrorMessage(error: string): string {
  if (error.includes('permission') || error.includes('access')) {
    return 'Could not save your API key. Please check that the app has permission to store data.';
  }

  if (error.includes('storage') || error.includes('write')) {
    return 'Could not save your API key. There may be a problem with your disk or storage.';
  }

  return 'Failed to save API key. Please try again.';
}
