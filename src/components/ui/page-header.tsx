/**
 * PageHeader — the one h1 of a screen, with optional eyebrow, description,
 * meta row, actions and a back link. The back link's `label` is REQUIRED and
 * is its visible text (no icon-only "Back" with a mismatched aria-label).
 * Sizes: `app` (owner screens), `doc` (public docs-style pages such as /cli),
 * `compact` (dense screens such as the dashboard).
 */
import { ArrowLeft } from 'lucide-react'
import type { ReactNode } from 'react'
import { Link } from 'react-router-dom'
import { cn } from '@/lib/cn'
import { Overline } from './overline'

export type PageHeaderSize = 'app' | 'doc' | 'compact'

const TITLE_CLASSES: Record<PageHeaderSize, string> = {
  app: 'text-h1',
  doc: 'text-display-sm',
  compact: 'text-h2',
}

export interface PageHeaderProps {
  title: ReactNode
  description?: ReactNode
  eyebrow?: ReactNode
  meta?: ReactNode
  actions?: ReactNode
  back?: { to: string; label: string }
  size?: PageHeaderSize
  className?: string
}

export function PageHeader({
  title,
  description,
  eyebrow,
  meta,
  actions,
  back,
  size = 'app',
  className,
}: PageHeaderProps) {
  return (
    <header className={cn('space-y-3', className)}>
      {back && (
        <Link
          to={back.to}
          className="inline-flex items-center gap-1.5 rounded-xs text-body-sm text-text-secondary transition-colors duration-base hover:text-text-primary"
        >
          <ArrowLeft className="h-4 w-4" aria-hidden="true" />
          {back.label}
        </Link>
      )}
      <div className="flex flex-wrap items-start justify-between gap-x-6 gap-y-3">
        <div className="min-w-0 flex-1 basis-72 space-y-1.5">
          {eyebrow && <Overline>{eyebrow}</Overline>}
          <h1 className={cn(TITLE_CLASSES[size], 'text-text-primary')}>{title}</h1>
          {description && (
            <p className={cn('text-text-secondary', size === 'doc' ? 'text-lead' : 'text-body-sm')}>
              {description}
            </p>
          )}
        </div>
        {actions && <div className="flex flex-wrap items-center gap-2">{actions}</div>}
      </div>
      {meta && (
        <div className="flex flex-wrap items-center gap-x-4 gap-y-2 text-body-sm text-text-secondary">{meta}</div>
      )}
    </header>
  )
}
