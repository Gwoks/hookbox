/** EmptyState — centered "nothing here yet" block: optional decorative icon,
 * heading, one line of guidance and an optional action. */
import type { ReactNode } from 'react'
import { cn } from '@/lib/cn'

export function EmptyState({
  icon,
  title,
  description,
  action,
  titleAs: Title = 'h2',
  className,
}: {
  icon?: ReactNode
  title: ReactNode
  description?: ReactNode
  action?: ReactNode
  titleAs?: 'h2' | 'h3' | 'h4'
  className?: string
}) {
  return (
    <div className={cn('flex flex-col items-center gap-2 px-6 py-12 text-center', className)}>
      {icon && (
        <div className="mb-1 text-text-tertiary" aria-hidden="true">
          {icon}
        </div>
      )}
      <Title className="text-h4 text-text-primary">{title}</Title>
      {description && <p className="max-w-sm text-body-sm text-text-secondary">{description}</p>}
      {action && <div className="pt-2">{action}</div>}
    </div>
  )
}
