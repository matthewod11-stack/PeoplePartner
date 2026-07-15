import { useEffect, useState } from 'react';
import type { Employee } from '../../../lib/types';
import {
  getEmployee,
  type EmployeeWithLatestRating,
} from '../../../lib/tauri-commands';

/**
 * Resolve a manager's display name for the employee detail panel.
 *
 * The loaded employee list is filtered and capped (`listEmployeesWithRatings`
 * runs with the active filter/search and a limit of 200), so a manager can
 * legitimately be absent from it — e.g. the list is filtered to active staff
 * while the manager is on leave, or the org exceeds the page cap. Before, the
 * panel fell back to rendering the raw manager ID in that case (issue #150).
 *
 * When the manager is present in the loaded list we use it directly; when it is
 * not, we fetch it by ID via the existing `get_employee` command (no backend
 * change). Returns `undefined` until a name is available, so the caller only
 * falls back to the raw ID for a genuinely dangling reference.
 */
export function useResolvedManagerName(
  managerId: string | null | undefined,
  employees: EmployeeWithLatestRating[]
): string | undefined {
  const managerInList = managerId
    ? employees.find((e) => e.id === managerId)
    : undefined;

  const [fetchedManager, setFetchedManager] = useState<Employee | null>(null);

  useEffect(() => {
    // Resolvable from the loaded list, or no manager at all — nothing to fetch.
    if (!managerId || managerInList) {
      setFetchedManager(null);
      return;
    }

    let cancelled = false;
    getEmployee(managerId)
      .then((emp) => {
        if (!cancelled) setFetchedManager(emp);
      })
      .catch(() => {
        // Genuinely dangling reference — leave null so the caller falls back
        // to the raw ID (mirrors the backend's skip-not-fatal behavior).
        if (!cancelled) setFetchedManager(null);
      });

    return () => {
      cancelled = true;
    };
  }, [managerId, managerInList]);

  return (managerInList ?? fetchedManager)?.full_name;
}
