/**
 * Page sections. `SettingsSection` is one titled white Card per settings
 * group; `scroll-mt-nav` keeps an anchor jump from hiding its heading under
 * the 64px sticky nav. `MarketingSection` is a public-page band with
 * DESIGN.md's ~80px vertical rhythm.
 */
import { useId, type ReactNode } from 'react'
import { cn } from '@/lib/cn'
import { Card } from './card'
import { Overline } from './overline'

export function SettingsSection({
  id,
  title,
  description,
  actions,
  tone = 'default',
  className,
  children,
}: {
  id?: string
  title: ReactNode
  description?: ReactNode
  actions?: ReactNode
  tone?: 'default' | 'danger'
  className?: string
  children: ReactNode
}) {
  const headingId = useId()
  const descriptionId = useId()
  const isDanger = tone === 'danger'
  return (
    <Card
      as="section"
      id={id}
      aria-labelledby={headingId}
      aria-describedby={description ? descriptionId : undefined}
      className={cn('scroll-mt-nav', isDanger && 'border-danger-border', className)}
    >
      <div className="mb-5 flex flex-wrap items-start justify-between gap-3">
        <div className="min-w-0 space-y-1">
          <h2 id={headingId} className={cn('text-h4', isDanger ? 'text-danger-fg' : 'text-text-primary')}>
            {title}
          </h2>
          {description && (
            <p id={descriptionId} className="text-body-sm text-text-secondary">
              {description}
            </p>
          )}
        </div>
        {actions}
      </div>
      <div className="space-y-4">{children}</div>
    </Card>
  )
}

export function MarketingSection({
  id,
  eyebrow,
  title,
  description,
  align = 'center',
  className,
  children,
}: {
  id?: string
  eyebrow?: ReactNode
  title?: ReactNode
  description?: ReactNode
  align?: 'center' | 'start'
  className?: string
  children?: ReactNode
}) {
  const headingId = useId()
  const hasIntro = Boolean(eyebrow || title || description)
  return (
    <section id={id} aria-labelledby={title ? headingId : undefined} className={cn('py-20', className)}>
      {hasIntro && (
        <div className={cn('mb-10 max-w-doc space-y-3', align === 'center' && 'mx-auto text-center')}>
          {eyebrow && <Overline>{eyebrow}</Overline>}
          {title && (
            <h2 id={headingId} className="text-display-sm text-text-primary">
              {title}
            </h2>
          )}
          {description && <p className="text-lead text-text-secondary">{description}</p>}
        </div>
      )}
      {children}
    </section>
  )
}
