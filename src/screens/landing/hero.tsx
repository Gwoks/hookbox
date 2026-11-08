/**
 * The landing hero — DESIGN.md's centered stack: a row of method "character
 * marks" (decorative: aria-hidden, a staggered spring entrance, no motion
 * at all under reduced motion), the display headline with its peach
 * highlight pill, the lead, then the call to action.
 */
import type { CSSProperties } from 'react'
import { t } from '@/lib/copy'
import { MethodBadge } from '@/components/hookbox/method-badge'
import { GetStarted } from './get-started'
import { HighlightText } from './highlight-text'

const MARKS = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE'] as const
const MARK_STAGGER_MS = 70

export function Hero({ hasResumeFailed, areCookiesEnabled }: { hasResumeFailed: boolean; areCookiesEnabled: boolean }) {
  return (
    <section className="flex flex-col items-center gap-6 pb-12 pt-10 text-center sm:pt-16">
      <div className="flex flex-wrap justify-center gap-2" aria-hidden="true">
        {MARKS.map((method, i) => (
          <span
            key={method}
            className="inline-flex animate-mark-in motion-reduce:animate-none"
            style={{ animationDelay: `${i * MARK_STAGGER_MS}ms` } as CSSProperties}
          >
            <MethodBadge method={method} />
          </span>
        ))}
      </div>
      <h1 className="max-w-wide text-balance text-display-sm text-text-primary md:text-display lg:text-display-lg">
        <HighlightText text={t('landing.hero.headline')} highlight={t('landing.hero.highlight')} />
      </h1>
      <p className="max-w-doc text-lead text-text-secondary">{t('landing.hero.subhead')}</p>
      <GetStarted hasResumeFailed={hasResumeFailed} areCookiesEnabled={areCookiesEnabled} />
    </section>
  )
}
