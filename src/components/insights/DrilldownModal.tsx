/**
 * DrilldownModal Component (V2.3.2l)
 *
 * Modal showing filtered employee list when clicking on a chart segment.
 * Allows viewing employees that match the clicked dimension.
 */

import { useState, useEffect } from 'react';
import { Modal } from '../shared/Modal';
import { listEmployees, type EmployeeFilter } from '../../lib/tauri-commands';
import type { Employee } from '../../lib/types';

interface DrilldownModalProps {
  /** Whether modal is open */
  isOpen: boolean;
  /** Called when modal should close */
  onClose: () => void;
  /** Filter for employees */
  filter: EmployeeFilter;
  /** Display label (e.g., "Department: Engineering") */
  label: string;
  /** Called when an employee is selected (optional) */
  onSelectEmployee?: (employee: Employee) => void;
}

export function DrilldownModal({
  isOpen,
  onClose,
  filter,
  label,
  onSelectEmployee,
}: DrilldownModalProps) {
  const [employees, setEmployees] = useState<Employee[]>([]);
  const [total, setTotal] = useState(0);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Fetch employees when modal opens
  useEffect(() => {
    if (!isOpen) return;

    const fetchEmployees = async () => {
      setIsLoading(true);
      setError(null);
      try {
        const result = await listEmployees(filter, 100, 0);
        setEmployees(result.employees);
        setTotal(result.total);
      } catch (err) {
        setError(err instanceof Error ? err.message : 'Failed to load employees');
      } finally {
        setIsLoading(false);
      }
    };

    fetchEmployees();
  }, [isOpen, filter]);

  const handleSelectEmployee = (employee: Employee) => {
    if (onSelectEmployee) {
      onSelectEmployee(employee);
      onClose();
    }
  };

  return (
    <Modal isOpen={isOpen} onClose={onClose} maxWidth="max-w-2xl">
      <div className="space-y-4">
        {/* Header */}
        <div className="border-b border-stone-200 pb-4">
          <h2 className="text-lg font-semibold text-stone-900">{label}</h2>
          <p className="text-sm text-stone-500 mt-1">
            {isLoading ? 'Loading...' : `${total} employee${total === 1 ? '' : 's'}`}
          </p>
        </div>

        {/* Content */}
        {isLoading ? (
          <div className="py-12 text-center text-stone-500">Loading employees...</div>
        ) : error ? (
          <div className="py-12 text-center text-red-600">{error}</div>
        ) : employees.length === 0 ? (
          <div className="py-12 text-center">
            <p className="text-stone-600 font-medium mb-1">No employees found</p>
            <p className="text-sm text-stone-500">
              No employees match the selected criteria.
            </p>
          </div>
        ) : (
          <div className="max-h-[400px] overflow-y-auto">
            <table className="w-full">
              <thead className="sticky top-0 bg-stone-50">
                <tr className="text-left text-xs font-medium text-stone-500 uppercase tracking-wider">
                  <th className="px-4 py-2">Name</th>
                  <th className="px-4 py-2">Department</th>
                  <th className="px-4 py-2">Title</th>
                  <th className="px-4 py-2">Status</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-stone-100">
                {employees.map((employee) => (
                  <tr
                    key={employee.id}
                    className={`
                      hover:bg-stone-50 transition-colors
                      ${onSelectEmployee ? 'cursor-pointer' : ''}
                    `}
                    onClick={() => handleSelectEmployee(employee)}
                  >
                    <td className="px-4 py-3">
                      <div className="font-medium text-stone-900">{employee.full_name}</div>
                      <div className="text-xs text-stone-500">{employee.email}</div>
                    </td>
                    <td className="px-4 py-3 text-sm text-stone-600">
                      {employee.department || '—'}
                    </td>
                    <td className="px-4 py-3 text-sm text-stone-600">
                      {employee.job_title || '—'}
                    </td>
                    <td className="px-4 py-3">
                      <StatusBadge status={employee.status} />
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
            {total > 100 && (
              <p className="text-center text-xs text-stone-500 py-3 border-t border-stone-100">
                Showing first 100 of {total} employees
              </p>
            )}
          </div>
        )}
      </div>
    </Modal>
  );
}

function StatusBadge({ status }: { status: string }) {
  const styles: Record<string, string> = {
    active: 'bg-green-100 text-green-700',
    terminated: 'bg-red-100 text-red-700',
    leave: 'bg-amber-100 text-amber-700',
  };

  return (
    <span
      className={`
        inline-flex px-2 py-0.5 rounded-md text-xs font-medium
        ${styles[status] || 'bg-stone-100 text-stone-600'}
      `}
    >
      {status.charAt(0).toUpperCase() + status.slice(1)}
    </span>
  );
}

export default DrilldownModal;
