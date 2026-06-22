/** InlineAlert (design.md §3.12). Left-rail-accented panel, radius-md, icon +
 * message (+ optional action). Variants info/warn/danger — fg for icon +
 * heading, soft bg fill. Persistent (rate-limit/429, endpoint-gone,
 * storage-unavailable, webhook "stored not sent", pending inspector). */
import { AlertTriangle, Info, ShieldAlert, type LucideIcon } from 'lucide-react'
import type { ReactNode } from 'react'
import { cn } from '@/lib/cn'

type Variant = 'info' | 'warning' | 'danger'

const CONFIG: Record<Variant, { icon: LucideIcon; rail: string; fg: string; bg: string }> = {
  info: { icon: Info, rail: 'border-l-info-fg', fg: 'text-info-fg', bg: 'bg-info-bg' },
  warning: { icon: AlertTriangle, rail: 'border-l-warning-fg', fg: 'text-warning-fg', bg: 'bg-warning-bg' },
  danger: { icon: ShieldAlert, rail: 'border-l-danger-fg', fg: 'text-danger-fg', bg: 'bg-danger-bg' },
}

export function InlineAlert({
  variant = 'info',
  title,
  children,
  action,
  className,
  role,
}: {
  variant?: Variant
  title?: ReactNode
  children?: ReactNode
  action?: ReactNode
  className?: string
  role?: 'alert' | 'status'
}) {
  const c = CONFIG[variant]
  const Icon = c.icon
  return (
    <div
      role={role}
      className={cn('flex items-start gap-2.5 rounded-md border-l-2 p-3', c.rail, c.bg, className)}
    >
      <Icon className={cn('mt-0.5 h-4 w-4 shrink-0', c.fg)} aria-hidden="true" />
      <div className="min-w-0 flex-1 space-y-1">
        {title && <p className={cn('text-body-sm font-semibold', c.fg)}>{title}</p>}
        {children && <div className="text-body-sm text-text-secondary">{children}</div>}
        {action && <div className="pt-1">{action}</div>}
      </div>
    </div>
  )
}
