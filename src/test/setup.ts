// Global test setup — loaded before every test file (see vitest.config.ts).
// Extends `expect` with jest-dom matchers (toBeInTheDocument, toBeDisabled, …).
import '@testing-library/jest-dom/vitest';
import { afterEach } from 'vitest';
import { clearMocks } from '@tauri-apps/api/mocks';

// Reset any Tauri IPC mocks between tests so command handlers don't leak.
afterEach(() => {
  clearMocks();
});
