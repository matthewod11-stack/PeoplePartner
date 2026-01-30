/**
 * Drilldown Utilities (V2.3.2l)
 *
 * Maps chart GroupBy dimensions to EmployeeFilter for drilldown functionality.
 */

import type { GroupBy } from './analytics-types';
import type { EmployeeFilter } from './tauri-commands';

/** Result of building a drilldown filter */
export type DrilldownResult =
  | { type: 'filter'; filter: EmployeeFilter; label: string }
  | { type: 'unavailable'; reason: string };

/**
 * Maps a GroupBy dimension and clicked label to an EmployeeFilter.
 * Some dimensions (tenure_bucket, rating_bucket) cannot be directly filtered.
 */
export function buildEmployeeFilter(groupBy: GroupBy, label: string): DrilldownResult {
  switch (groupBy) {
    case 'department':
      return {
        type: 'filter',
        filter: { department: label },
        label: `Department: ${label}`,
      };

    case 'status':
      // Normalize status label to lowercase for filter
      const statusMap: Record<string, 'active' | 'terminated' | 'leave'> = {
        Active: 'active',
        active: 'active',
        Terminated: 'terminated',
        terminated: 'terminated',
        Leave: 'leave',
        leave: 'leave',
        'On Leave': 'leave',
      };
      const status = statusMap[label];
      if (!status) {
        return {
          type: 'unavailable',
          reason: `Unknown status: ${label}`,
        };
      }
      return {
        type: 'filter',
        filter: { status },
        label: `Status: ${label}`,
      };

    case 'gender':
      return {
        type: 'filter',
        filter: { gender: label },
        label: `Gender: ${label}`,
      };

    case 'ethnicity':
      return {
        type: 'filter',
        filter: { ethnicity: label },
        label: `Ethnicity: ${label}`,
      };

    case 'work_state':
      return {
        type: 'filter',
        filter: { work_state: label },
        label: `Work State: ${label}`,
      };

    case 'tenure_bucket':
      return {
        type: 'unavailable',
        reason: 'Tenure-based drilldown requires date calculations not available in employee filter.',
      };

    case 'rating_bucket':
      return {
        type: 'unavailable',
        reason: 'Rating-based drilldown requires joining performance ratings table.',
      };

    case 'quarter':
      return {
        type: 'unavailable',
        reason: 'Quarter-based drilldown requires date range filtering.',
      };

    default:
      return {
        type: 'unavailable',
        reason: `Unknown grouping: ${groupBy}`,
      };
  }
}

/**
 * Checks if a GroupBy dimension supports drilldown.
 */
export function isDrilldownSupported(groupBy: GroupBy): boolean {
  const supported: GroupBy[] = ['department', 'status', 'gender', 'ethnicity', 'work_state'];
  return supported.includes(groupBy);
}
