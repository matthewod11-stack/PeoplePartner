/**
 * Avatar Component
 *
 * Displays user initials in a circular container with size and color variants.
 * Follows the People Partner "Warm Editorial" design aesthetic.
 */

import { getInitials } from './utils';

export interface AvatarProps {
  /** Full name (used to generate initials) */
  name: string;
  /** Size variant */
  size?: 'sm' | 'md' | 'lg';
  /** Color variant */
  variant?: 'default' | 'primary';
  /** Additional CSS classes */
  className?: string;
}

const sizeClasses = {
  sm: 'w-8 h-8 text-xs',
  md: 'w-10 h-10 text-sm',
  lg: 'w-14 h-14 text-lg',
} as const;

const variantClasses = {
  default: 'bg-stone-100 text-stone-600',
  primary: 'bg-primary-100 text-primary-600',
} as const;

/**
 * Avatar displays user initials in a circular badge.
 *
 * @example
 * // Default medium avatar
 * <Avatar name="John Doe" />
 *
 * @example
 * // Large primary avatar for detail views
 * <Avatar name="Jane Smith" size="lg" variant="primary" />
 */
export function Avatar({
  name,
  size = 'md',
  variant = 'default',
  className = '',
}: AvatarProps) {
  const initials = getInitials(name);

  return (
    <div
      className={`
        ${sizeClasses[size]}
        ${variantClasses[variant]}
        rounded-full
        flex items-center justify-center
        font-medium
        flex-shrink-0
        ${className}
      `}
      aria-label={name}
      title={name}
    >
      {initials}
    </div>
  );
}

export default Avatar;
