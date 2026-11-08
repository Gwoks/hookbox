/** RetryAlert — the load-failed InlineAlert with an outline Retry button
 * (previously hand-built in seven places). role="alert" by default so the
 * failure is announced; omit `onRetry` for a terminal message. */
import { RotateCw } from 'lucide-react'
import type { ReactNode } from 'react'
import { t } from '@/lib/copy'
import { Button } from '@/components/ui/button'
import { InlineAlert } from './inline-alert'

export function RetryAlert({
  title,
  children,
  onRetry,
  retrying = false,
  retryLabel = t('common.retry'),
  variant = 'danger',
  role = 'alert',
  className,
}: {
  title?: ReactNode
  children?: ReactNode
  onRetry?: () => void
  retrying?: boolean
  retryLabel?: string
  variant?: 'danger' | 'warning' | 'info'
  role?: 'alert' | 'status'
  className?: string
}) {
  return (
    <InlineAlert
      variant={variant}
      role={role}
      title={title}
      className={className}
      action={
        onRetry && (
          <Button variant="outline" size="sm" onClick={onRetry} loading={retrying}>
            {!retrying && <RotateCw className="h-3.5 w-3.5" aria-hidden="true" />}
            {retryLabel}
          </Button>
        )
      }
    >
      {children}
    </InlineAlert>
  )
}
