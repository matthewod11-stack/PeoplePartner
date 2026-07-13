/**
 * Employee Header Component
 *
 * Header section for EmployeeDetail showing avatar, name, status, and quick stats.
 */

import type { Employee, PerformanceRating, EnpsResponse } from '../../../lib/types';
import { Avatar, StatusBadge, calculateTenure, getRatingColor, getEnpsColor } from '../../ui';

interface EmployeeHeaderProps {
  employee: Employee;
  latestRating?: PerformanceRating;
  latestEnps?: EnpsResponse;
  onEdit?: () => void;
  /** Opens the Prep Brief modal. Not rendered for sample employees (#118 —
   *  sample data never uses API credits), matching the backend guard. */
  onPrepBrief?: () => void;
}

/**
 * Header section with avatar, name, status badge, and quick stats.
 */
export function EmployeeHeader({
  employee,
  latestRating,
  latestEnps,
  onEdit,
  onPrepBrief,
}: EmployeeHeaderProps) {
  return (
    <div className="p-4 border-b border-stone-200/60">
      <div className="flex items-start gap-3">
        {/* Avatar */}
        <Avatar name={employee.full_name} size="lg" variant="primary" />

        {/* Name & title */}
        <div className="flex-1 min-w-0">
          <h3 className="font-medium text-stone-800 text-lg truncate">
            {employee.full_name}
          </h3>
          {employee.job_title && (
            <p className="text-sm text-stone-500 truncate">
              {employee.job_title}
            </p>
          )}
          <div className="flex items-center gap-2 mt-1.5">
            <StatusBadge status={employee.status} size="sm" />
            {employee.department && (
              <span className="text-xs text-stone-500">
                {employee.department}
              </span>
            )}
          </div>
        </div>

        {/* Prep Brief — hidden for sample employees (#118) */}
        {onPrepBrief && !employee.is_sample && (
          <button
            onClick={onPrepBrief}
            className="px-2.5 py-1.5 text-xs font-medium text-teal-700 bg-teal-50 hover:bg-teal-100 rounded-lg transition-colors self-start"
            aria-label={`Generate prep brief for ${employee.full_name}`}
          >
            Prep Brief
          </button>
        )}

        {/* Edit button */}
        {onEdit && (
          <button
            onClick={onEdit}
            className="p-2.5 text-stone-500 hover:text-stone-700 hover:bg-stone-100 rounded-lg transition-colors"
            aria-label="Edit employee"
          >
            <svg
              className="w-4 h-4"
              fill="none"
              viewBox="0 0 24 24"
              stroke="currentColor"
              strokeWidth={2}
            >
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                d="M16.862 4.487l1.687-1.688a1.875 1.875 0 112.652 2.652L10.582 16.07a4.5 4.5 0 01-1.897 1.13L6 18l.8-2.685a4.5 4.5 0 011.13-1.897l8.932-8.931zm0 0L19.5 7.125M18 14v4.75A2.25 2.25 0 0115.75 21H5.25A2.25 2.25 0 013 18.75V8.25A2.25 2.25 0 015.25 6H10"
              />
            </svg>
          </button>
        )}
      </div>

      {/* Quick stats */}
      <div className="flex gap-4 mt-4">
        {latestRating && (
          <div className="text-center">
            <div
              className={`text-lg font-semibold ${getRatingColor(latestRating.overall_rating).split(' ')[0]}`}
            >
              {latestRating.overall_rating.toFixed(1)}
            </div>
            <div className="text-xs text-stone-500">Rating</div>
          </div>
        )}
        {latestEnps && (
          <div className="text-center">
            <div
              className={`text-lg font-semibold ${getEnpsColor(latestEnps.score).split(' ')[0]}`}
            >
              {latestEnps.score}
            </div>
            <div className="text-xs text-stone-500">eNPS</div>
          </div>
        )}
        <div className="text-center">
          <div className="text-lg font-semibold text-stone-700">
            {calculateTenure(employee.hire_date)}
          </div>
          <div className="text-xs text-stone-500">Tenure</div>
        </div>
      </div>
    </div>
  );
}
