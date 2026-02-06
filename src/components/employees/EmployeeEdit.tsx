import { useState, useEffect, useCallback } from 'react';
import type { Employee } from '../../lib/types';
import { updateEmployee, type UpdateEmployeeInput } from '../../lib/tauri-commands';
import { Modal } from '../shared/Modal';

// =============================================================================
// Types
// =============================================================================

interface EmployeeEditProps {
  employee: Employee;
  isOpen: boolean;
  onClose: () => void;
  onSave: (updated: Employee) => void;
}

interface FormData {
  full_name: string;
  email: string;
  job_title: string;
  department: string;
  work_state: string;
  hire_date: string;
  status: 'active' | 'terminated' | 'leave';
  date_of_birth: string;
  gender: string;
  ethnicity: string;
  termination_date: string;
  termination_reason: string;
}

// =============================================================================
// Helper Components
// =============================================================================

interface FormFieldProps {
  label: string;
  name: string;
  value: string;
  onChange: (name: string, value: string) => void;
  type?: 'text' | 'email' | 'date' | 'select';
  options?: { value: string; label: string }[];
  required?: boolean;
  placeholder?: string;
}

function FormField({
  label,
  name,
  value,
  onChange,
  type = 'text',
  options,
  required,
  placeholder,
}: FormFieldProps) {
  const baseClassName = `
    w-full px-3 py-2
    bg-white border border-stone-200
    rounded-lg text-sm text-stone-700
    focus:outline-none focus:ring-2 focus:ring-primary-500/20 focus:border-primary-400
    transition-all duration-200
  `;

  const fieldId = `employee-edit-${name}`;

  return (
    <div>
      <label htmlFor={fieldId} className="block text-sm font-medium text-stone-600 mb-1">
        {label}
        {required && <span className="text-red-500 ml-1">*</span>}
      </label>
      {type === 'select' && options ? (
        <select
          id={fieldId}
          name={name}
          value={value}
          onChange={(e) => onChange(name, e.target.value)}
          className={baseClassName}
        >
          {options.map((opt) => (
            <option key={opt.value} value={opt.value}>
              {opt.label}
            </option>
          ))}
        </select>
      ) : (
        <input
          id={fieldId}
          type={type}
          name={name}
          value={value}
          onChange={(e) => onChange(name, e.target.value)}
          placeholder={placeholder}
          required={required}
          className={baseClassName}
        />
      )}
    </div>
  );
}

function SectionDivider({ title }: { title: string }) {
  return (
    <div className="flex items-center gap-3 pt-4 pb-2">
      <span className="text-xs font-medium text-stone-500 uppercase tracking-wider">
        {title}
      </span>
      <div className="flex-1 h-px bg-stone-200" />
    </div>
  );
}

// =============================================================================
// Main Component
// =============================================================================

export function EmployeeEdit({ employee, isOpen, onClose, onSave }: EmployeeEditProps) {
  const [formData, setFormData] = useState<FormData>({
    full_name: '',
    email: '',
    job_title: '',
    department: '',
    work_state: '',
    hire_date: '',
    status: 'active',
    date_of_birth: '',
    gender: '',
    ethnicity: '',
    termination_date: '',
    termination_reason: '',
  });
  const [isSaving, setIsSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Initialize form with employee data
  useEffect(() => {
    if (employee && isOpen) {
      setFormData({
        full_name: employee.full_name || '',
        email: employee.email || '',
        job_title: employee.job_title || '',
        department: employee.department || '',
        work_state: employee.work_state || '',
        hire_date: employee.hire_date || '',
        status: employee.status || 'active',
        date_of_birth: employee.date_of_birth || '',
        gender: employee.gender || '',
        ethnicity: employee.ethnicity || '',
        termination_date: employee.termination_date || '',
        termination_reason: employee.termination_reason || '',
      });
      setError(null);
    }
  }, [employee, isOpen]);

  // Handle field changes
  const handleChange = useCallback((name: string, value: string) => {
    setFormData((prev) => ({ ...prev, [name]: value }));
    setError(null);
  }, []);

  // Handle form submission
  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setIsSaving(true);
    setError(null);

    try {
      // Build update input (only include changed fields)
      const input: UpdateEmployeeInput = {};

      if (formData.full_name !== employee.full_name) input.full_name = formData.full_name;
      if (formData.email !== employee.email) input.email = formData.email;
      if (formData.job_title !== (employee.job_title || '')) input.job_title = formData.job_title || undefined;
      if (formData.department !== (employee.department || '')) input.department = formData.department || undefined;
      if (formData.work_state !== (employee.work_state || '')) input.work_state = formData.work_state || undefined;
      if (formData.hire_date !== (employee.hire_date || '')) input.hire_date = formData.hire_date || undefined;
      if (formData.status !== employee.status) input.status = formData.status;
      if (formData.date_of_birth !== (employee.date_of_birth || '')) input.date_of_birth = formData.date_of_birth || undefined;
      if (formData.gender !== (employee.gender || '')) input.gender = formData.gender || undefined;
      if (formData.ethnicity !== (employee.ethnicity || '')) input.ethnicity = formData.ethnicity || undefined;
      if (formData.termination_date !== (employee.termination_date || '')) input.termination_date = formData.termination_date || undefined;
      if (formData.termination_reason !== (employee.termination_reason || '')) input.termination_reason = formData.termination_reason || undefined;

      const updated = await updateEmployee(employee.id, input);
      onSave(updated);
      onClose();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to save changes');
    } finally {
      setIsSaving(false);
    }
  };

  if (!isOpen) return null;

  const statusOptions = [
    { value: 'active', label: 'Active' },
    { value: 'leave', label: 'On Leave' },
    { value: 'terminated', label: 'Terminated' },
  ];

  const terminationReasonOptions = [
    { value: '', label: 'Select reason...' },
    { value: 'voluntary', label: 'Voluntary' },
    { value: 'involuntary', label: 'Involuntary' },
    { value: 'retirement', label: 'Retirement' },
    { value: 'other', label: 'Other' },
  ];

  return (
    <Modal isOpen={isOpen} onClose={onClose} title="Edit Employee" maxWidth="max-w-3xl">
      <form onSubmit={handleSubmit} className="space-y-4">
        {/* Error message */}
        {error && (
          <div className="p-3 bg-red-50 border border-red-200 rounded-lg text-sm text-red-600" role="alert" aria-live="assertive">
            {error}
          </div>
        )}

        {/* Basic Info */}
        <div className="grid grid-cols-2 gap-4">
          <FormField
            label="Full Name"
            name="full_name"
            value={formData.full_name}
            onChange={handleChange}
            required
          />
          <FormField
            label="Email"
            name="email"
            type="email"
            value={formData.email}
            onChange={handleChange}
            required
          />
        </div>

        <div className="grid grid-cols-2 gap-4">
          <FormField
            label="Job Title"
            name="job_title"
            value={formData.job_title}
            onChange={handleChange}
            placeholder="e.g., Software Engineer"
          />
          <FormField
            label="Department"
            name="department"
            value={formData.department}
            onChange={handleChange}
            placeholder="e.g., Engineering"
          />
        </div>

        <div className="grid grid-cols-2 gap-4">
          <FormField
            label="Work State"
            name="work_state"
            value={formData.work_state}
            onChange={handleChange}
            placeholder="e.g., California"
          />
          <FormField
            label="Hire Date"
            name="hire_date"
            type="date"
            value={formData.hire_date}
            onChange={handleChange}
          />
        </div>

        <FormField
          label="Status"
          name="status"
          type="select"
          value={formData.status}
          onChange={handleChange}
          options={statusOptions}
        />

        {/* Termination fields (shown when status is terminated) */}
        {formData.status === 'terminated' && (
          <div className="grid grid-cols-2 gap-4 p-4 bg-stone-50 rounded-lg">
            <FormField
              label="Termination Date"
              name="termination_date"
              type="date"
              value={formData.termination_date}
              onChange={handleChange}
            />
            <FormField
              label="Reason"
              name="termination_reason"
              type="select"
              value={formData.termination_reason}
              onChange={handleChange}
              options={terminationReasonOptions}
            />
          </div>
        )}

        <SectionDivider title="Demographics" />

        <div className="grid grid-cols-3 gap-4">
          <FormField
            label="Date of Birth"
            name="date_of_birth"
            type="date"
            value={formData.date_of_birth}
            onChange={handleChange}
          />
          <FormField
            label="Gender"
            name="gender"
            value={formData.gender}
            onChange={handleChange}
            placeholder="Optional"
          />
          <FormField
            label="Ethnicity"
            name="ethnicity"
            value={formData.ethnicity}
            onChange={handleChange}
            placeholder="Optional"
          />
        </div>

        {/* Footer */}
        <div className="pt-4 border-t border-stone-200 flex items-center justify-end gap-3 bg-stone-50 px-4 py-3 rounded-lg">
          <button
            type="button"
            onClick={onClose}
            className="px-4 py-2 text-sm font-medium text-stone-600 hover:text-stone-800 hover:bg-stone-100 rounded-lg transition-colors"
          >
            Cancel
          </button>
          <button
            type="submit"
            disabled={isSaving}
            className="
              px-4 py-2 text-sm font-medium text-white
              bg-primary-500 hover:bg-primary-600
              rounded-lg transition-all
              disabled:opacity-50 disabled:cursor-not-allowed
              flex items-center gap-2
            "
          >
            {isSaving && (
              <svg className="w-4 h-4 animate-spin-slow" viewBox="0 0 24 24" fill="none" aria-hidden="true">
                <circle cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="3" className="opacity-25" />
                <path d="M12 2a10 10 0 0110 10" stroke="currentColor" strokeWidth="3" strokeLinecap="round" className="opacity-75" />
              </svg>
            )}
            {isSaving ? 'Saving...' : 'Save Changes'}
          </button>
        </div>
      </form>
    </Modal>
  );
}

export default EmployeeEdit;
