/**
 * Button Component
 *
 * Multi-variant button with consistent hover states and accessibility.
 * Follows the People Partner "Warm Editorial" design aesthetic.
 */

import { forwardRef } from 'react';

export interface ButtonProps
  extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  /** Visual variant */
  variant?: 'primary' | 'secondary' | 'ghost' | 'icon' | 'link';
  /** Size */
  size?: 'sm' | 'md' | 'lg';
  /** Full width */
  fullWidth?: boolean;
  /** Loading state */
  isLoading?: boolean;
  /** Left icon slot */
  leftIcon?: React.ReactNode;
  /** Right icon slot */
  rightIcon?: React.ReactNode;
}

const variantClasses = {
  primary: `
    bg-primary-500 hover:bg-primary-600 active:bg-primary-700
    text-white font-medium
    shadow-sm hover:shadow-md active:shadow-sm
    hover:brightness-110 active:brightness-95
    transition-all duration-200 ease-smooth-out
  `,
  secondary: `
    bg-white border border-stone-200
    text-stone-700 hover:text-stone-800 font-medium
    hover:bg-stone-50 hover:border-stone-300
    shadow-sm hover:shadow-md active:shadow-sm
    hover:brightness-105 active:brightness-95
    transition-all duration-200 ease-smooth-out
  `,
  ghost: `
    text-stone-600 hover:text-stone-800
    hover:bg-stone-100
    transition-colors duration-200
  `,
  icon: `
    text-stone-500 hover:text-stone-700
    hover:bg-stone-200/60
    hover:brightness-110 active:brightness-90
    transition-all duration-200
  `,
  link: `
    text-primary-600 hover:text-primary-700
    underline-offset-2 hover:underline
    transition-colors duration-150
  `,
} as const;

const sizeClasses = {
  sm: 'px-3 py-1.5 text-sm rounded-lg gap-1.5',
  md: 'px-4 py-2 text-sm rounded-lg gap-2',
  lg: 'px-6 py-3 text-base rounded-xl gap-2',
} as const;

// Icon variant has fixed sizing for touch targets
const iconSizeClasses = {
  sm: 'w-8 h-8 rounded-md',
  md: 'w-10 h-10 rounded-lg',
  lg: 'w-12 h-12 rounded-lg',
} as const;

/**
 * Button with multiple visual variants and sizes.
 *
 * @example
 * // Primary action button
 * <Button variant="primary" onClick={handleSubmit}>
 *   Save Changes
 * </Button>
 *
 * @example
 * // Secondary with icon
 * <Button variant="secondary" leftIcon={<PlusIcon />}>
 *   Add Employee
 * </Button>
 *
 * @example
 * // Icon-only button (for toolbars)
 * <Button variant="icon" aria-label="Settings">
 *   <GearIcon />
 * </Button>
 *
 * @example
 * // Link-style button
 * <Button variant="link">Learn more</Button>
 */
export const Button = forwardRef<HTMLButtonElement, ButtonProps>(
  function Button(
    {
      children,
      variant = 'primary',
      size = 'md',
      fullWidth = false,
      isLoading = false,
      leftIcon,
      rightIcon,
      disabled,
      className = '',
      ...props
    },
    ref
  ) {
    const isIconVariant = variant === 'icon';
    const isLinkVariant = variant === 'link';

    // Build class string
    const classes = [
      // Base styles
      'inline-flex items-center justify-center',
      'font-medium',
      'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-500 focus-visible:ring-offset-2',

      // Variant styles
      variantClasses[variant],

      // Size styles (icon variant uses fixed sizes)
      isIconVariant
        ? iconSizeClasses[size]
        : isLinkVariant
          ? '' // Links don't need padding
          : sizeClasses[size],

      // Width
      fullWidth && 'w-full',

      // Disabled state
      (disabled || isLoading) &&
        'opacity-50 cursor-not-allowed pointer-events-none',

      // Custom classes
      className,
    ]
      .filter(Boolean)
      .join(' ');

    return (
      <button
        ref={ref}
        disabled={disabled || isLoading}
        className={classes}
        {...props}
      >
        {isLoading ? (
          <LoadingSpinner />
        ) : (
          <>
            {leftIcon && <span className="flex-shrink-0">{leftIcon}</span>}
            {children}
            {rightIcon && <span className="flex-shrink-0">{rightIcon}</span>}
          </>
        )}
      </button>
    );
  }
);

/**
 * Simple loading spinner for button loading state.
 */
function LoadingSpinner() {
  return (
    <svg
      className="animate-spin-slow h-4 w-4"
      xmlns="http://www.w3.org/2000/svg"
      fill="none"
      viewBox="0 0 24 24"
    >
      <circle
        className="opacity-25"
        cx="12"
        cy="12"
        r="10"
        stroke="currentColor"
        strokeWidth="4"
      />
      <path
        className="opacity-75"
        fill="currentColor"
        d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
      />
    </svg>
  );
}

export default Button;
