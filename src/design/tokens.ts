/**
 * Design tokens — the single source for the Tailwind theme scales
 * (tailwind.config.ts) AND the tailwind-merge class groups (src/lib/cn.ts),
 * so a new size/shadow/radius key can never exist in one and be unknown to
 * the other (unknown keys made `cn()` drop classes it misread as colors).
 *
 * Pure data with NO imports: tailwind.config.ts loads this under Node and the
 * app bundle loads it in the browser. Colors are deliberately absent — they
 * are theme-aware CSS custom properties defined in src/globals.css; values
 * that change per theme or under reduced motion (shadows, durations,
 * easings) are referenced here as `var(--…)` for the same reason.
 *
 * Type scale: DESIGN.md's "Notion" scale adapted to an app's density —
 * negative tracking on display sizes, normal tracking on body sizes.
 */

interface FontSizeOptions {
  lineHeight: string
  fontWeight?: string
  letterSpacing?: string
}
type FontSizeToken = [size: string, options: FontSizeOptions]

const FONT_SIZE = {
  'display-lg': ['72px', { lineHeight: '1.15', fontWeight: '700', letterSpacing: '-0.028em' }],
  display: ['54px', { lineHeight: '1.04', fontWeight: '700', letterSpacing: '-0.035em' }],
  'display-sm': ['40px', { lineHeight: '1.1', fontWeight: '700', letterSpacing: '-0.03em' }],
  h1: ['24px', { lineHeight: '1.25', fontWeight: '700', letterSpacing: '-0.015em' }],
  h2: ['22px', { lineHeight: '1.27', fontWeight: '600', letterSpacing: '-0.011em' }],
  h3: ['20px', { lineHeight: '1.3', fontWeight: '600' }],
  h4: ['16px', { lineHeight: '1.4', fontWeight: '600' }],
  lead: ['18px', { lineHeight: '1.56', fontWeight: '400' }],
  body: ['16px', { lineHeight: '1.5' }],
  'body-sm': ['14px', { lineHeight: '1.43' }],
  caption: ['12px', { lineHeight: '1.33', fontWeight: '500', letterSpacing: '0.01em' }],
  overline: ['12px', { lineHeight: '1.33', fontWeight: '600', letterSpacing: '0.06em' }],
  mono: ['13px', { lineHeight: '1.5', fontWeight: '450' }],
  'mono-sm': ['12px', { lineHeight: '1.45', fontWeight: '450' }],
  'mono-lg': ['15px', { lineHeight: '1.4', fontWeight: '500' }],
} satisfies Record<string, FontSizeToken>

export type FontSizeKey = keyof typeof FONT_SIZE
export const fontSize: Record<FontSizeKey, FontSizeToken> = FONT_SIZE

/** Content cards are hairline-bordered (DESIGN.md "Don't"): shadows are for
 * floating surfaces (menus, dialogs, toasts), the sticky nav and the
 * landing's product mockup only. */
export const boxShadow = {
  xs: 'var(--shadow-xs)',
  sm: 'var(--shadow-sm)',
  md: 'var(--shadow-md)',
  lg: 'var(--shadow-lg)',
  focus: 'var(--shadow-focus)',
  nav: 'var(--shadow-nav)',
  mockup: 'var(--shadow-mockup)',
}

/** DESIGN.md: small 4px, buttons 8px, cards 12px, pills 9999px — and nothing
 * larger than 12px on rectangular content, so there is no `lg` tier. */
export const borderRadius = {
  xs: '4px',
  sm: '8px',
  md: '12px',
  pill: '9999px',
}

export const maxWidth = {
  page: '1440px',
  wide: '1120px',
  content: '920px',
  doc: '720px',
  form: '560px',
  // Legacy keys still used by the pre-rebuild screens (removed in S5).
  landing: '560px',
  'landing-hero': '1040px',
  settings: '640px',
  viewer: '920px',
}

export const spacing = {
  /** Sticky top-nav height (DESIGN.md: fixed 64px bar). */
  nav: '64px',
}

export const transitionDuration = {
  instant: 'var(--dur-instant)',
  fast: 'var(--dur-fast)',
  base: 'var(--dur-base)',
  slow: 'var(--dur-slow)',
}

/** `spring` is for decorative marks only (DESIGN.md); the reduced-motion block
 * in globals.css neutralises it. */
export const transitionTimingFunction = {
  standard: 'var(--ease-standard)',
  emphasized: 'var(--ease-emphasized)',
  spring: 'var(--ease-spring)',
}

export const zIndex = {
  sticky: '100',
  nav: '200',
  dropdown: '1000',
  popover: '1100',
  dialog: '1200',
  toast: '1300',
  tooltip: '1400',
}
