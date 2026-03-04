/**
 * Badge Component
 *
 * Status indicators, rating badges, and tag-like elements.
 * Includes convenience components for common patterns.
 */

import type { BadgeVariant } from './utils';
import {
  getRatingVariant,
  getEnpsVariant,
  getStatusBadge as getStatusConfig,
} from './utils';

export interface BadgeProps {
  /** Badge content */
  children: React.ReactNode;
  /** Visual variant */
  variant?: BadgeVariant;
  /** Size */
  size?: 'sm' | 'md';
  /** Pill shape (fully rounded) */
  pill?: boolean;
  /** Additional CSS classes */
  className?: string;
}

const variantClasses: Record<BadgeVariant, string> = {
  default: 'bg-stone-100 text-stone-600',
  success: 'bg-green-100 text-green-700',
  warning: 'bg-amber-100 text-amber-700',
  error: 'bg-red-100 text-red-700',
  info: 'bg-blue-100 text-blue-700',
};

const sizeClasses = {
  sm: 'px-1.5 py-0.5 text-xs',
  md: 'px-2 py-0.5 text-sm',
} as const;

/**
 * Badge displays status, category, or label information.
 *
 * @example
 * <Badge variant="success">Active</Badge>
 * <Badge variant="warning" pill>Pending</Badge>
 */
export function Badge({
  children,
  variant = 'default',
  size = 'md',
  pill = false,
  className = '',
}: BadgeProps) {
  return (
    <span
      className={`
        inline-flex items-center
        ${sizeClasses[size]}
        ${variantClasses[variant]}
        ${pill ? 'rounded-full' : 'rounded'}
        font-medium
        ${className}
      `}
    >
      {children}
    </span>
  );
}

// =============================================================================
// Convenience Components
// =============================================================================

export interface StatusBadgeProps {
  /** Employee status */
  status: 'active' | 'terminated' | 'leave' | string;
  /** Size */
  size?: 'sm' | 'md';
  /** Additional CSS classes */
  className?: string;
}

/**
 * StatusBadge auto-colors based on employee status.
 *
 * @example
 * <StatusBadge status="active" />
 * <StatusBadge status="terminated" size="sm" />
 */
export function StatusBadge({
  status,
  size = 'md',
  className = '',
}: StatusBadgeProps) {
  const config = getStatusConfig(status);
  return (
    <Badge variant={config.variant} size={size} pill className={className}>
      {config.label}
    </Badge>
  );
}

export interface RatingBadgeProps {
  /** Rating value (1.0 - 5.0) */
  rating: number;
  /** Show decimal value */
  showValue?: boolean;
  /** Size */
  size?: 'sm' | 'md';
  /** Additional CSS classes */
  className?: string;
}

/**
 * RatingBadge auto-colors based on rating value.
 *
 * @example
 * <RatingBadge rating={4.5} />
 * <RatingBadge rating={3.2} showValue />
 */
export function RatingBadge({
  rating,
  showValue = true,
  size = 'md',
  className = '',
}: RatingBadgeProps) {
  const variant = getRatingVariant(rating);
  return (
    <Badge variant={variant} size={size} className={className}>
      {showValue ? rating.toFixed(1) : null}
    </Badge>
  );
}

export interface EnpsBadgeProps {
  /** eNPS score (0-10) */
  score: number;
  /** Show the score value */
  showValue?: boolean;
  /** Size */
  size?: 'sm' | 'md';
  /** Additional CSS classes */
  className?: string;
}

/**
 * EnpsBadge auto-colors based on eNPS score.
 *
 * @example
 * <EnpsBadge score={9} />
 * <EnpsBadge score={6} showValue />
 */
export function EnpsBadge({
  score,
  showValue = true,
  size = 'md',
  className = '',
}: EnpsBadgeProps) {
  const variant = getEnpsVariant(score);
  return (
    <Badge variant={variant} size={size} className={className}>
      {showValue ? score : null}
    </Badge>
  );
}

export default Badge;
