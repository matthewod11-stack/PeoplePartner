import { defineConfig } from 'vitest/config';
import react from '@vitejs/plugin-react';

// Frontend test floor (#115). Kept separate from vite.config.ts so the app
// build config and the test config don't fight over defineConfig typings.
export default defineConfig({
  plugins: [react()],
  test: {
    environment: 'jsdom',
    globals: true,
    setupFiles: ['./src/test/setup.ts'],
    include: ['src/**/*.{test,spec}.{ts,tsx}'],
    css: false,
    coverage: {
      provider: 'v8',
      reportsDirectory: './coverage',
      reporter: ['text-summary', 'html'],
      // Floor areas only — no numeric gate yet (see #115 verification).
      include: ['src/lib/**', 'src/contexts/**', 'src/hooks/**'],
    },
  },
});
