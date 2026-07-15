import { renderHook, waitFor } from '@testing-library/react';
import { describe, it, expect } from 'vitest';
import { mockCommands } from '../../../test/tauri';
import type { EmployeeWithLatestRating } from '../../../lib/tauri-commands';
import { useResolvedManagerName } from './useResolvedManagerName';

function emp(
  overrides: Partial<EmployeeWithLatestRating> = {}
): EmployeeWithLatestRating {
  return {
    id: 'emp-1',
    email: 'a@example.com',
    full_name: 'Ada Example',
    status: 'active',
    is_sample: false,
    created_at: '2026-01-01',
    updated_at: '2026-01-01',
    ...overrides,
  };
}

describe('useResolvedManagerName', () => {
  it('resolves the name from the loaded list when the manager is present', () => {
    const employees = [emp({ id: 'mgr-1', full_name: 'Priya Raman' })];
    const { result } = renderHook(() =>
      useResolvedManagerName('mgr-1', employees)
    );
    expect(result.current).toBe('Priya Raman');
  });

  it('fetches the manager by ID when it is absent from the filtered/capped list', async () => {
    // #150: manager exists but is not in the loaded (filtered, limit-200) list.
    mockCommands({
      get_employee: (args) => {
        expect(args.id).toBe('mgr-hidden');
        return emp({ id: 'mgr-hidden', full_name: 'Priya Raman' });
      },
    });
    const employees = [emp({ id: 'emp-2', full_name: 'Someone Else' })];
    const { result } = renderHook(() =>
      useResolvedManagerName('mgr-hidden', employees)
    );
    // Not in the list -> undefined initially, resolved after the fetch settles.
    expect(result.current).toBeUndefined();
    await waitFor(() => expect(result.current).toBe('Priya Raman'));
  });

  it('returns undefined when there is no manager', () => {
    const { result } = renderHook(() => useResolvedManagerName(undefined, []));
    expect(result.current).toBeUndefined();
  });

  it('falls back to undefined (raw-ID caller path) when the fetch fails', async () => {
    mockCommands({
      get_employee: () => {
        throw new Error('not found');
      },
    });
    const { result } = renderHook(() =>
      useResolvedManagerName('mgr-dangling', [emp({ id: 'emp-2' })])
    );
    await waitFor(() => expect(result.current).toBeUndefined());
  });
});
