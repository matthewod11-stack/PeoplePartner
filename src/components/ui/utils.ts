/**
 * Shared UI Utility Functions
 *
 * Common helpers used across UI components for formatting,
 * color mapping, and display logic.
 */

// =============================================================================
// Text Formatting
// =============================================================================

/**
 * Generate 2-letter initials from a full name.
 * @example getInitials("John Doe") => "JD"
 */
export function getInitials(name: string): string {
  return name
    .split(' ')
    .map((part) => part[0])
    .join('')
    .toUpperCase()
    .slice(0, 2);
}

/**
 * Format a date string for display.
 * @example formatDate("2024-01-15") => "Jan 15, 2024"
 */
export function formatDate(dateStr?: string): string {
  if (!dateStr) return '—';
  try {
    const date = new Date(dateStr);
    return date.toLocaleDateString('en-US', {
      month: 'short',
      day: 'numeric',
      year: 'numeric'
    });
  } catch {
    return dateStr;
  }
}

/**
 * Calculate tenure from hire date as a human-readable string.
 * @example calculateTenure("2022-06-01") => "2y 6m"
 */
export function calculateTenure(hireDate?: string): string {
  if (!hireDate) return '—';
  try {
    const hire = new Date(hireDate);
    const now = new Date();
    const years = Math.floor(
      (now.getTime() - hire.getTime()) / (365.25 * 24 * 60 * 60 * 1000)
    );
    const months = Math.floor(
      ((now.getTime() - hire.getTime()) % (365.25 * 24 * 60 * 60 * 1000)) /
        (30.44 * 24 * 60 * 60 * 1000)
    );

    if (years > 0) {
      return `${years}y ${months}m`;
    }
    return `${months}m`;
  } catch {
    return '—';
  }
}

// =============================================================================
// Badge Variants
// =============================================================================

export type BadgeVariant = 'default' | 'success' | 'warning' | 'error' | 'info';

/**
 * Get badge variant based on performance rating value.
 * - 4.5+ = success (green)
 * - 3.5+ = info (blue)
 * - 2.5+ = warning (amber)
 * - below = error (red)
 */
export function getRatingVariant(rating: number): BadgeVariant {
  if (rating >= 4.5) return 'success';
  if (rating >= 3.5) return 'info';
  if (rating >= 2.5) return 'warning';
  return 'error';
}

/**
 * Get Tailwind classes for rating display (legacy, for gradual migration).
 */
export function getRatingColor(rating: number): string {
  if (rating >= 4.5) return 'text-green-600 bg-green-50';
  if (rating >= 3.5) return 'text-blue-600 bg-blue-50';
  if (rating >= 2.5) return 'text-amber-600 bg-amber-50';
  return 'text-red-600 bg-red-50';
}

/**
 * Get badge variant based on eNPS score.
 * - 9-10 = success (promoter)
 * - 7-8 = warning (passive)
 * - 0-6 = error (detractor)
 */
export function getEnpsVariant(score: number): BadgeVariant {
  if (score >= 9) return 'success';
  if (score >= 7) return 'warning';
  return 'error';
}

/**
 * Get Tailwind classes for eNPS display (legacy, for gradual migration).
 */
export function getEnpsColor(score: number): string {
  if (score >= 9) return 'text-green-600 bg-green-50';
  if (score >= 7) return 'text-amber-600 bg-amber-50';
  return 'text-red-600 bg-red-50';
}

/**
 * Get badge variant based on employee status.
 */
export function getStatusVariant(
  status: string
): BadgeVariant {
  switch (status) {
    case 'active':
      return 'success';
    case 'leave':
      return 'warning';
    case 'terminated':
    default:
      return 'default';
  }
}

/**
 * Get status badge configuration (label + classes).
 */
export function getStatusBadge(status: string): {
  label: string;
  className: string;
  variant: BadgeVariant;
} {
  switch (status) {
    case 'active':
      return {
        label: 'Active',
        className: 'bg-primary-100 text-primary-700',
        variant: 'success'
      };
    case 'terminated':
      return {
        label: 'Terminated',
        className: 'bg-stone-100 text-stone-600',
        variant: 'default'
      };
    case 'leave':
      return {
        label: 'On Leave',
        className: 'bg-amber-100 text-amber-700',
        variant: 'warning'
      };
    default:
      return {
        label: status,
        className: 'bg-stone-100 text-stone-600',
        variant: 'default'
      };
  }
}

/**
 * Get status indicator dot configuration (for list items).
 */
export function getStatusIndicator(status: string): {
  color: string;
  label: string;
} {
  switch (status) {
    case 'active':
      return { color: 'bg-primary-500', label: 'Active' };
    case 'terminated':
      return { color: 'bg-stone-400', label: 'Terminated' };
    case 'leave':
      return { color: 'bg-amber-500', label: 'On Leave' };
    default:
      return { color: 'bg-stone-300', label: status };
  }
}
