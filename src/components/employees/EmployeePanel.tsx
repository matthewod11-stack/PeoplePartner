import { useState, useMemo, useEffect } from 'react';
import { useEmployees } from '../../contexts/EmployeeContext';
import { useTrial } from '../../contexts/TrialContext';
import { RATING_LABELS } from '../../lib/types';
import { getDepartments } from '../../lib/tauri-commands';
import { Avatar, getStatusIndicator, getRatingColor } from '../ui';

// =============================================================================
// Helper Components
// =============================================================================

function SearchInput({
  value,
  onChange,
  placeholder = 'Search employees...',
}: {
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
}) {
  return (
    <div className="relative">
      <svg
        className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-stone-500"
        fill="none"
        viewBox="0 0 24 24"
        stroke="currentColor"
        strokeWidth={1.5}
      >
        <path
          strokeLinecap="round"
          strokeLinejoin="round"
          d="M21 21l-5.197-5.197m0 0A7.5 7.5 0 105.196 5.196a7.5 7.5 0 0010.607 10.607z"
        />
      </svg>
      <input
        type="text"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
        className="
          w-full pl-9 pr-3 py-2
          bg-white/60 border border-stone-200/60
          rounded-lg
          text-sm text-stone-700 placeholder:text-stone-400
          focus:outline-none focus:ring-2 focus:ring-primary-500/20 focus:border-primary-400
          transition-all duration-200
        "
      />
      {value && (
        <button
          onClick={() => onChange('')}
          className="absolute right-1 top-1/2 -translate-y-1/2 p-2 text-stone-500 hover:text-stone-700 rounded-md"
          aria-label="Clear search"
        >
          <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
            <path strokeLinecap="round" strokeLinejoin="round" d="M6 18L18 6M6 6l12 12" />
          </svg>
        </button>
      )}
    </div>
  );
}

type StatusFilter = 'all' | 'active' | 'terminated' | 'leave';

function StatusFilterTabs({
  value,
  onChange,
  counts,
}: {
  value: StatusFilter;
  onChange: (value: StatusFilter) => void;
  counts: { all: number; active: number; terminated: number; leave: number };
}) {
  const tabs: { key: StatusFilter; label: string }[] = [
    { key: 'all', label: 'All' },
    { key: 'active', label: 'Active' },
    { key: 'terminated', label: 'Left' },
    { key: 'leave', label: 'Leave' },
  ];

  return (
    <div className="flex gap-1 p-1 bg-stone-100/60 rounded-lg">
      {tabs.map((tab) => (
        <button
          key={tab.key}
          onClick={() => onChange(tab.key)}
          className={`
            flex-1 px-2 py-1.5 text-xs font-medium rounded-md
            transition-all duration-200
            ${
              value === tab.key
                ? 'bg-white text-stone-700 shadow-sm'
                : 'text-stone-500 hover:text-stone-700 hover:bg-white/50'
            }
          `}
        >
          {tab.label}
          <span className="ml-1 text-stone-500">
            {counts[tab.key]}
          </span>
        </button>
      ))}
    </div>
  );
}

function FilterDropdown({
  label,
  value,
  options,
  onChange,
}: {
  label: string;
  value: string;
  options: { value: string; label: string }[];
  onChange: (value: string) => void;
}) {
  return (
    <div className="flex-1 min-w-0">
      <select
        value={value}
        onChange={(e) => onChange(e.target.value)}
        className="
          w-full px-2 py-1.5
          bg-white/60 border border-stone-200/60
          rounded-lg
          text-xs text-stone-700
          focus:outline-none focus:ring-2 focus:ring-primary-500/20 focus:border-primary-400
          transition-all duration-200
          cursor-pointer
        "
        aria-label={label}
      >
        <option value="">{label}</option>
        {options.map((opt) => (
          <option key={opt.value} value={opt.value}>
            {opt.label}
          </option>
        ))}
      </select>
    </div>
  );
}

// Helper functions imported from ../ui: getStatusIndicator, getRatingColor

interface EmployeeCardProps {
  name: string;
  title?: string;
  department?: string;
  status: string;
  rating?: number;
  isSelected: boolean;
  onClick: () => void;
}

function EmployeeCard({
  name,
  title,
  department,
  status,
  rating,
  isSelected,
  onClick,
}: EmployeeCardProps) {
  const statusIndicator = getStatusIndicator(status);

  return (
    <button
      onClick={onClick}
      className={`
        w-full p-3 rounded-lg text-left
        transition-all duration-200
        ${
          isSelected
            ? 'bg-primary-50 border border-primary-200 shadow-sm'
            : 'bg-white/60 border border-transparent hover:bg-white hover:border-stone-200/60 hover:shadow-sm'
        }
      `}
      aria-pressed={isSelected}
    >
      <div className="flex items-start gap-3">
        {/* Avatar */}
        <Avatar
          name={name}
          size="md"
          variant={isSelected ? 'primary' : 'default'}
        />

        {/* Info */}
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2">
            <p className="font-medium text-stone-800 truncate">{name}</p>
            {/* Status dot */}
            <span
              className={`w-2 h-2 rounded-full flex-shrink-0 ${statusIndicator.color}`}
              title={statusIndicator.label}
            />
          </div>

          {title && (
            <p className="text-sm text-stone-500 truncate mt-0.5">{title}</p>
          )}

          <div className="flex items-center gap-2 mt-1.5">
            {department && (
              <span className="text-xs text-stone-500 truncate">
                {department}
              </span>
            )}

            {rating !== undefined && (
              <span
                className={`
                  ml-auto px-1.5 py-0.5 rounded text-xs font-medium
                  ${getRatingColor(rating)}
                `}
                title={RATING_LABELS[Math.round(rating)] ?? `Rating: ${rating}`}
              >
                {rating.toFixed(1)}
              </span>
            )}
          </div>
        </div>
      </div>
    </button>
  );
}

// =============================================================================
// Main Component
// =============================================================================

export function EmployeePanel() {
  const {
    employees,
    selectedEmployeeId,
    totalCount,
    isLoading,
    error,
    searchQuery,
    setSearchQuery,
    setFilter,
    selectEmployee,
    openImportWizard,
  } = useEmployees();

  const { isTrialMode, trialStatus } = useTrial();

  const [statusFilter, setStatusFilter] = useState<StatusFilter>('all');
  const [departmentFilter, setDepartmentFilter] = useState('');
  const [managerFilter, setManagerFilter] = useState('');
  const [departments, setDepartments] = useState<string[]>([]);

  // Fetch departments on mount
  useEffect(() => {
    getDepartments()
      .then(setDepartments)
      .catch((err) => console.error('Failed to load departments:', err));
  }, []);

  // Build manager options from employees (only those who are managers)
  const managerOptions = useMemo(() => {
    const managerIds = new Set(employees.map((e) => e.manager_id).filter(Boolean));
    return employees
      .filter((e) => managerIds.has(e.id))
      .map((e) => ({ value: e.id, label: e.full_name }))
      .sort((a, b) => a.label.localeCompare(b.label));
  }, [employees]);

  // Calculate counts for tabs
  const statusCounts = useMemo(() => {
    return {
      all: totalCount,
      active: employees.filter((e) => e.status === 'active').length,
      terminated: employees.filter((e) => e.status === 'terminated').length,
      leave: employees.filter((e) => e.status === 'leave').length,
    };
  }, [employees, totalCount]);

  // Build and apply combined filter
  const applyFilters = (
    status: StatusFilter,
    department: string,
    manager: string
  ) => {
    const filter: Record<string, string | undefined> = {};
    if (status !== 'all') filter.status = status;
    if (department) filter.department = department;
    // Note: manager_id isn't in backend filter, handled via client-side filtering
    setFilter(filter);
    setManagerFilter(manager);
  };

  // Update filter when status tab changes
  const handleStatusChange = (status: StatusFilter) => {
    setStatusFilter(status);
    applyFilters(status, departmentFilter, managerFilter);
  };

  const handleDepartmentChange = (department: string) => {
    setDepartmentFilter(department);
    applyFilters(statusFilter, department, managerFilter);
  };

  const handleManagerChange = (manager: string) => {
    setManagerFilter(manager);
    applyFilters(statusFilter, departmentFilter, manager);
  };

  // Client-side filter for manager (backend doesn't support manager_id filter)
  const filteredEmployees = useMemo(() => {
    if (!managerFilter) return employees;
    return employees.filter((e) => e.manager_id === managerFilter);
  }, [employees, managerFilter]);

  // Loading state
  if (isLoading) {
    return (
      <div className="h-full flex flex-col p-4">
        <div className="animate-pulse space-y-4">
          <div className="h-10 bg-stone-200/60 rounded-lg" />
          <div className="h-8 bg-stone-200/60 rounded-lg" />
          <div className="space-y-2 mt-4">
            {[1, 2, 3, 4, 5].map((i) => (
              <div key={i} className="h-20 bg-stone-200/40 rounded-lg" />
            ))}
          </div>
        </div>
      </div>
    );
  }

  // Error state
  if (error) {
    return (
      <div className="h-full flex flex-col p-4">
        <div className="flex-1 flex flex-col items-center justify-center text-center">
          <svg
            className="w-12 h-12 text-stone-300 mb-3"
            fill="none"
            viewBox="0 0 24 24"
            stroke="currentColor"
            strokeWidth={1}
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              d="M12 9v3.75m9-.75a9 9 0 11-18 0 9 9 0 0118 0zm-9 3.75h.008v.008H12v-.008z"
            />
          </svg>
          <p className="text-stone-500 text-sm">{error}</p>
        </div>
      </div>
    );
  }

  return (
    <div className="h-full flex flex-col p-4">
      {/* Search */}
      <div className="space-y-3">
        <SearchInput
          value={searchQuery}
          onChange={setSearchQuery}
        />

        {/* Status filter tabs */}
        <StatusFilterTabs
          value={statusFilter}
          onChange={handleStatusChange}
          counts={statusCounts}
        />

        {/* Department & Manager filters */}
        <div className="flex gap-2">
          <FilterDropdown
            label="All Departments"
            value={departmentFilter}
            options={departments.map((d) => ({ value: d, label: d }))}
            onChange={handleDepartmentChange}
          />
          <FilterDropdown
            label="All Managers"
            value={managerFilter}
            options={managerOptions}
            onChange={handleManagerChange}
          />
        </div>
      </div>

      {/* Employee count + Import button */}
      <div className="mt-4 mb-2">
        <div className="flex items-center justify-between">
          <p className="text-xs font-medium text-stone-500 uppercase tracking-wider">
            {filteredEmployees.length} {filteredEmployees.length === 1 ? 'Employee' : 'Employees'}
          </p>
          <button
            onClick={openImportWizard}
            className="text-xs text-primary-600 hover:text-primary-700 font-medium flex items-center gap-1"
          >
            <svg className="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
              <path strokeLinecap="round" strokeLinejoin="round" d="M3 16.5v2.25A2.25 2.25 0 005.25 21h13.5A2.25 2.25 0 0021 18.75V16.5m-13.5-9L12 3m0 0l4.5 4.5M12 3v13.5" />
            </svg>
            Import
          </button>
        </div>

        {/* Trial employee limit indicator */}
        {isTrialMode && trialStatus && (
          <div className="mt-1.5">
            <div className="flex justify-between text-xs text-stone-500">
              <span>{trialStatus.employees_used}/{trialStatus.employees_limit} employees</span>
              {trialStatus.employees_used >= trialStatus.employees_limit && (
                <span className="text-red-500 font-medium">Limit reached</span>
              )}
            </div>
            <div className="mt-1 h-1.5 bg-stone-200 rounded-full overflow-hidden">
              <div
                className={`h-full rounded-full transition-all ${
                  trialStatus.employees_used >= trialStatus.employees_limit
                    ? 'bg-red-500'
                    : trialStatus.employees_used >= trialStatus.employees_limit * 0.8
                      ? 'bg-amber-500'
                      : 'bg-primary-500'
                }`}
                style={{
                  width: `${Math.min(100, (trialStatus.employees_used / trialStatus.employees_limit) * 100)}%`,
                }}
              />
            </div>
          </div>
        )}
      </div>

      {/* Employee list */}
      <div className="flex-1 overflow-y-auto -mx-2 px-2 space-y-2">
        {filteredEmployees.length === 0 ? (
          <div className="flex flex-col items-center justify-center py-12 text-center">
            <svg
              className="w-12 h-12 text-stone-300 mb-3"
              fill="none"
              viewBox="0 0 24 24"
              stroke="currentColor"
              strokeWidth={1}
            >
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                d="M18 18.72a9.094 9.094 0 003.741-.479 3 3 0 00-4.682-2.72m.94 3.198l.001.031c0 .225-.012.447-.037.666A11.944 11.944 0 0112 21c-2.17 0-4.207-.576-5.963-1.584A6.062 6.062 0 016 18.719m12 0a5.971 5.971 0 00-.941-3.197m0 0A5.995 5.995 0 0012 12.75a5.995 5.995 0 00-5.058 2.772m0 0a3 3 0 00-4.681 2.72 8.986 8.986 0 003.74.477m.94-3.197a5.971 5.971 0 00-.94 3.197M15 6.75a3 3 0 11-6 0 3 3 0 016 0zm6 3a2.25 2.25 0 11-4.5 0 2.25 2.25 0 014.5 0zm-13.5 0a2.25 2.25 0 11-4.5 0 2.25 2.25 0 014.5 0z"
              />
            </svg>
            <p className="text-stone-500 text-sm">No employees found</p>
            <p className="text-stone-500 text-xs mt-1">
              {searchQuery ? 'Try a different search' : 'Import employees to get started'}
            </p>
          </div>
        ) : (
          filteredEmployees.map((employee) => (
            <EmployeeCard
              key={employee.id}
              name={employee.full_name}
              title={employee.job_title}
              department={employee.department}
              status={employee.status}
              rating={employee.latestRating?.overall_rating}
              isSelected={selectedEmployeeId === employee.id}
              onClick={() => selectEmployee(employee.id)}
            />
          ))
        )}
      </div>
    </div>
  );
}

export default EmployeePanel;
