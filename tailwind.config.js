/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {
      colors: {
        // Primary accent (10% of UI) — Complete teal scale
        primary: {
          50: '#F4F8F8',   // Lightest tint (backgrounds, hover states)
          100: '#E5EFF0',  // Highlights, selected states
          200: '#CAE1E2',  // Light accent backgrounds
          300: '#9CCFD3',  // Soft highlights, badges
          400: '#5BBBC3',  // Mid-tone accent
          500: '#2D9199',  // Primary actions, links (anchor — logo teal)
          600: '#237076',  // Hover states
          700: '#1F5256',  // Active/pressed states
          800: '#1A3A3D',  // Dark accent text on light backgrounds
          900: '#102123',  // Darkest tint (rare, high contrast)
        },
        // Warm neutrals (90% of UI)
        stone: {
          50: '#FAFAF9',   // Background (warm off-white)
          100: '#F5F5F4',  // Surface (cards, panels)
          200: '#E7E5E4',  // Borders
          400: '#A8A29E',  // Muted text (timestamps)
          500: '#78716C',  // Secondary text
          700: '#44403C',  // Primary text
          900: '#1C1917',  // Headings, emphasis
        },
        // Semantic colors
        success: '#22C55E',
        warning: '#F59E0B',
        error: '#EF4444',
      },
      fontFamily: {
        display: ['Fraunces', 'Georgia', 'serif'],
        sans: ['DM Sans', '-apple-system', 'BlinkMacSystemFont', 'system-ui', 'sans-serif'],
        mono: ['SF Mono', 'Menlo', 'Monaco', 'monospace'],
      },
      fontSize: {
        xs: ['12px', { lineHeight: '16px' }],
        sm: ['14px', { lineHeight: '20px' }],
        base: ['16px', { lineHeight: '24px' }],
        lg: ['18px', { lineHeight: '28px' }],
        xl: ['20px', { lineHeight: '28px' }],
      },
      borderRadius: {
        sm: '4px',
        md: '8px',
        lg: '12px',
        xl: '16px',
      },
      boxShadow: {
        sm: '0 1px 2px 0 rgb(0 0 0 / 0.05)',
        md: '0 4px 6px -1px rgb(0 0 0 / 0.1)',
        lg: '0 10px 15px -3px rgb(0 0 0 / 0.1), 0 4px 6px -4px rgb(0 0 0 / 0.1)',
        xl: '0 20px 25px -5px rgb(0 0 0 / 0.1), 0 8px 10px -6px rgb(0 0 0 / 0.1)',
        '2xl': '0 25px 50px -12px rgb(0 0 0 / 0.25)',
      },
      // Letter-spacing tokens for typographic control
      letterSpacing: {
        tight: '-0.025em',   // Headlines, display text
        wide: '0.025em',     // Buttons, labels
        wider: '0.05em',     // All-caps text, badges
      },
      // Custom easing curves for refined motion
      transitionTimingFunction: {
        'smooth-out': 'cubic-bezier(0.0, 0.0, 0.2, 1)',    // Quick start, gentle stop (entrances)
        'smooth-in': 'cubic-bezier(0.4, 0.0, 1, 1)',       // Gentle start, quick end (exits)
        'smooth-in-out': 'cubic-bezier(0.4, 0.0, 0.2, 1)', // Balanced (state changes)
      },
      width: {
        'sidebar': '240px',
        'context-panel': '280px',
      },
      spacing: {
        // Design spec uses 4px base
        '1': '4px',
        '2': '8px',
        '3': '12px',
        '4': '16px',
        '6': '24px',
        '8': '32px',
        '12': '48px',
      },
      animation: {
        'fade-in': 'fadeIn 0.2s ease-out',
        'spin-slow': 'spin 1.5s linear infinite',  // Calmer loading spinner (default is 1s)
      },
      keyframes: {
        fadeIn: {
          '0%': { opacity: '0', transform: 'translateY(-4px)' },
          '100%': { opacity: '1', transform: 'translateY(0)' },
        },
      },
    },
  },
  plugins: [
    require('@tailwindcss/typography'),
  ],
}
