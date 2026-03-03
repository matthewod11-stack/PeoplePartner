/**
 * UI Primitives
 *
 * Shared, reusable components following the People Partner design system.
 * Import from '@/components/ui' or '../ui' for consistent styling.
 */

// Components
export { Avatar, type AvatarProps } from './Avatar';
export {
  Badge,
  StatusBadge,
  RatingBadge,
  EnpsBadge,
  type BadgeProps,
  type StatusBadgeProps,
  type RatingBadgeProps,
  type EnpsBadgeProps,
} from './Badge';
export { Button, type ButtonProps } from './Button';
export { Card, type CardProps } from './Card';

// Utilities
export {
  // Text formatting
  getInitials,
  formatDate,
  calculateTenure,
  // Badge variants
  type BadgeVariant,
  getRatingVariant,
  getRatingColor,
  getEnpsVariant,
  getEnpsColor,
  getStatusVariant,
  getStatusBadge,
  getStatusIndicator,
} from './utils';
