/** HookBox brand lockup (design.md §8 / OQ-D2): inline-SVG hook glyph in
 * accent teal + wordmark. No external asset (offline-safe; replaces the old
 * plain blue text brand). Color via currentColor/token classes only (AC-D11). */
import { cn } from '@/lib/cn'
import { t } from '@/lib/copy'

export function BrandMark({ className }: { className?: string }) {
  return (
    <span className={cn('inline-flex items-center gap-2', className)}>
      <svg
        viewBox="0 0 24 24"
        className="h-6 w-6 text-accent"
        fill="none"
        stroke="currentColor"
        strokeWidth="2.2"
        strokeLinecap="round"
        strokeLinejoin="round"
        role="img"
        aria-label={t('landing.brand.markAlt')}
      >
        {/* A minimal hook glyph. */}
        <path d="M16 4v8a5 5 0 0 1-10 0" />
        <circle cx="16" cy="3.5" r="1.4" fill="currentColor" stroke="none" />
      </svg>
      <span className="text-h2 font-bold text-text-primary">{t('landing.brand.wordmark')}</span>
    </span>
  )
}
