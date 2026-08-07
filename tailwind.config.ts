/**
 * Tailwind config (design.md §2 / §10). Every color maps to a SEMANTIC CSS
 * custom property defined in src/globals.css, so light/dark switch via tokens
 * (AC-D12) — never a hardcoded inversion, never a raw hex in a component
 * (AC-D11). Components reference `bg-surface`, `text-secondary`, `accent`,
 * `method.get.fg`, `served.rule`, `neutral-chip.bg`, etc. — not palette values.
 *
 * Mechanism mirrors ../shortener-link/tailwind.config.ts verbatim; only the
 * token NAMES specific to HookBox (method.*, served.*, accent.fill,
 * neutral-chip.*) and the new motion tokens (dur-instant, ease-emphasized,
 * feed-row-in / rail-flash keyframes) are added.
 */
import type { Config } from 'tailwindcss'

const config: Config = {
  darkMode: 'class',
  content: ['./index.html', './src/**/*.{ts,tsx}'],
  theme: {
    extend: {
      colors: {
        canvas: 'var(--bg-canvas)',
        surface: {
          DEFAULT: 'var(--bg-surface)',
          raised: 'var(--bg-surface-raised)',
          subtle: 'var(--bg-subtle)',
          hover: 'var(--bg-hover)',
          active: 'var(--bg-active)',
        },
        border: {
          DEFAULT: 'var(--border-default)',
          strong: 'var(--border-strong)',
        },
        text: {
          primary: 'var(--text-primary)',
          secondary: 'var(--text-secondary)',
          tertiary: 'var(--text-tertiary)',
          'on-accent': 'var(--text-on-accent)',
        },
        accent: {
          DEFAULT: 'var(--accent)',
          hover: 'var(--accent-hover)',
          fill: 'var(--accent-fill)',
          'subtle-bg': 'var(--accent-subtle-bg)',
        },
        success: { fg: 'var(--success-fg)', bg: 'var(--success-bg)' },
        warning: { fg: 'var(--warning-fg)', bg: 'var(--warning-bg)' },
        danger: { fg: 'var(--danger-fg)', bg: 'var(--danger-bg)' },
        info: { fg: 'var(--info-fg)', bg: 'var(--info-bg)' },
        focus: 'var(--focus-ring)',
        // Neutral chip (default/echo served-by, redacted note, disabled rule).
        'neutral-chip': { fg: 'var(--neutral-chip-fg)', bg: 'var(--neutral-chip-bg)' },
        // Method colors (design.md §2.2) — MethodBadge never hardcodes hex.
        method: {
          get: { fg: 'var(--m-get-fg)', bg: 'var(--m-get-bg)' },
          post: { fg: 'var(--m-post-fg)', bg: 'var(--m-post-bg)' },
          put: { fg: 'var(--m-put-fg)', bg: 'var(--m-put-bg)' },
          patch: { fg: 'var(--m-patch-fg)', bg: 'var(--m-patch-bg)' },
          delete: { fg: 'var(--m-delete-fg)', bg: 'var(--m-delete-bg)' },
          head: { fg: 'var(--m-head-fg)', bg: 'var(--m-head-bg)' },
        },
        // Served-by chip palette (design.md §2.3, AC-56).
        served: {
          rule: { fg: 'var(--sv-rule-fg)', bg: 'var(--sv-rule-bg)' },
          crud: { fg: 'var(--sv-crud-fg)', bg: 'var(--sv-crud-bg)' },
          mitm: { fg: 'var(--sv-mitm-fg)', bg: 'var(--sv-mitm-bg)' },
          tunnel: { fg: 'var(--sv-tunnel-fg)', bg: 'var(--sv-tunnel-bg)' },
          default: { fg: 'var(--sv-default-fg)', bg: 'var(--sv-default-bg)' },
          cors: { fg: 'var(--sv-cors-fg)', bg: 'var(--sv-cors-bg)' },
          chaos: { fg: 'var(--sv-chaos-fg)', bg: 'var(--sv-chaos-bg)' },
          ratelimit: { fg: 'var(--sv-ratelimit-fg)', bg: 'var(--sv-ratelimit-bg)' },
        },
      },
      borderColor: {
        DEFAULT: 'var(--border-default)',
      },
      fontFamily: {
        sans: 'var(--font-sans)',
        mono: 'var(--font-mono)',
      },
      fontSize: {
        display: ['44px', { lineHeight: '1.05', fontWeight: '700', letterSpacing: '-0.02em' }],
        h1: ['30px', { lineHeight: '1.2', fontWeight: '700' }],
        h2: ['22px', { lineHeight: '1.25', fontWeight: '650' }],
        h3: ['18px', { lineHeight: '1.3', fontWeight: '600' }],
        h4: ['15px', { lineHeight: '1.4', fontWeight: '600' }],
        body: ['15px', { lineHeight: '1.5' }],
        'body-sm': ['13px', { lineHeight: '1.45' }],
        caption: ['12px', { lineHeight: '1.4', fontWeight: '500' }],
        overline: ['11px', { lineHeight: '1.3', fontWeight: '600', letterSpacing: '0.06em' }],
        mono: ['13px', { lineHeight: '1.5', fontWeight: '450' }],
        'mono-sm': ['12px', { lineHeight: '1.45', fontWeight: '450' }],
        'mono-lg': ['16px', { lineHeight: '1.4', fontWeight: '500' }],
      },
      borderRadius: {
        xs: '4px',
        sm: '6px',
        md: '10px',
        lg: '14px',
        pill: '9999px',
      },
      boxShadow: {
        xs: 'var(--shadow-xs)',
        sm: 'var(--shadow-sm)',
        md: 'var(--shadow-md)',
        lg: 'var(--shadow-lg)',
        focus: 'var(--shadow-focus)',
      },
      maxWidth: {
        landing: '560px',
        'landing-hero': '1040px',
        settings: '640px',
        // The public /s/:code viewer's content column (design.md §2.4,
        // operator-toolkit F4) — shared by the banner, main and footer so
        // all three align.
        viewer: '920px',
      },
      minWidth: {
        feed: '360px',
      },
      spacing: {
        header: '52px',
        subheader: '48px',
      },
      transitionTimingFunction: {
        standard: 'cubic-bezier(.2,0,0,1)',
        emphasized: 'cubic-bezier(.2,.7,0,1)',
      },
      transitionDuration: {
        instant: '90ms',
        fast: '120ms',
        base: '180ms',
        slow: '240ms',
      },
      zIndex: {
        sticky: '100',
        nav: '200',
        dropdown: '1000',
        popover: '1100',
        dialog: '1200',
        toast: '1300',
        tooltip: '1400',
      },
      keyframes: {
        'fade-in': { from: { opacity: '0' }, to: { opacity: '1' } },
        'overlay-in': { from: { opacity: '0' }, to: { opacity: '1' } },
        'content-in': {
          from: { opacity: '0', transform: 'translateY(4px) scale(.98)' },
          to: { opacity: '1', transform: 'translateY(0) scale(1)' },
        },
        'sheet-in': {
          from: { transform: 'translateY(100%)' },
          to: { transform: 'translateY(0)' },
        },
        'toast-in': {
          from: { opacity: '0', transform: 'translateX(8px)' },
          to: { opacity: '1', transform: 'translateX(0)' },
        },
        // The signature feed new-row arrival (design.md §6 / §10).
        'feed-row-in': {
          from: { opacity: '0', transform: 'translateY(-8px)' },
          to: { opacity: '1', transform: 'translateY(0)' },
        },
        'rail-flash': {
          '0%': { boxShadow: 'inset 3px 0 0 var(--accent)' },
          '100%': { boxShadow: 'inset 3px 0 0 transparent' },
        },
        shimmer: { '100%': { transform: 'translateX(100%)' } },
        spin: { to: { transform: 'rotate(360deg)' } },
      },
      animation: {
        'fade-in': 'fade-in var(--dur-base) var(--ease-standard)',
        'overlay-in': 'overlay-in var(--dur-base) var(--ease-standard)',
        'content-in': 'content-in var(--dur-base) var(--ease-standard)',
        'sheet-in': 'sheet-in var(--dur-slow) var(--ease-standard)',
        'toast-in': 'toast-in var(--dur-base) var(--ease-standard)',
        'feed-row-in':
          'feed-row-in var(--dur-base) var(--ease-emphasized), rail-flash 600ms var(--ease-standard)',
        spin: 'spin 1s linear infinite',
      },
    },
  },
  plugins: [],
}

export default config
