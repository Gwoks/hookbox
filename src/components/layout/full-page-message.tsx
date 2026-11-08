/** FullPageMessage — a centered whole-page state (404, endpoint gone, a
 * share link that no longer resolves, a load failure with retry). Fills the
 * space its parent gives it; wrap in PublicPage for a standalone page. */
import type { ReactNode } from 'react'
import { cn } from '@/lib/cn'

export function FullPageMessage({
  icon,
  title,
  description,
  actions,
  className,
}: {
  icon?: ReactNode
  title: ReactNode
  description?: ReactNode
  actions?: ReactNode
  className?: string
}) {
  return (
    <div className={cn('flex flex-1 flex-col items-center justify-center gap-3 px-6 py-16 text-center', className)}>
      {icon && (
        <div className="text-text-tertiary" aria-hidden="true">
          {icon}
        </div>
      )}
      <h1 className="text-h2 text-text-primary">{title}</h1>
      {description && <p className="max-w-md text-body-sm text-text-secondary">{description}</p>}
      {actions && <div className="flex flex-wrap items-center justify-center gap-2 pt-2">{actions}</div>}
    </div>
  )
}
