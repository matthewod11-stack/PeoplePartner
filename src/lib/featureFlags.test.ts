import { describe, it, expect } from 'vitest';
import { RECRUITING_ENABLED } from './featureFlags';

// Regression lock for the recruiting production gate (#109).
//
// RECRUITING_ENABLED must stay `false` on any build that ships to users until
// the module is complete AND its Rust IPC mirror is flipped in lockstep
// (src-tauri/src/commands/recruiting.rs). This test is the frontend half of
// that gate: an accidental flip fails CI here, mirroring the Rust-side
// include_str! sweep test. When the module legitimately graduates, flip both
// flags and update this expectation deliberately.
describe('RECRUITING_ENABLED gate', () => {
  it('is OFF in the committed source', () => {
    expect(RECRUITING_ENABLED).toBe(false);
  });
});
