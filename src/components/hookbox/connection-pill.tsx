/** ConnectionPill (design.md §3.3, AC-41/43/D17). radius-pill, icon + colored
 * dot + text label; role="status" aria-live="polite". The TEXT LABEL carries the
 * state — the spin/dot color is never the sole cue (AC-D17). Copy strings
 * (feed.conn.*) are supplied by the caller (the feed hook, issue .28) so no copy
 * lives in this primitive. */
import {
  Loader2,
  Radio,
  RotateCw,
  Wifi,
  WifiOff,
  ShieldAlert,
  Ban,
  type LucideIcon,
} from 'lucide-react'
import { cn } from '@/lib/cn'

export type ConnState =
  | 'connecting'
  | 'live'
  | 'reconnecting'
  | 'degraded'
  | 'sse'
  | 'offline'
  | 'unauthorized'
  | 'busy'

const CONFIG: Record<ConnState, { icon: LucideIcon; cls: string; spin?: boolean }> = {
  connecting: { icon: Loader2, cls: 'text-text-tertiary', spin: true },
  live: { icon: Wifi, cls: 'text-success-fg' },
  reconnecting: { icon: RotateCw, cls: 'text-warning-fg', spin: true },
  degraded: { icon: RotateCw, cls: 'text-warning-fg', spin: true },
  sse: { icon: Radio, cls: 'text-info-fg' },
  offline: { icon: WifiOff, cls: 'text-danger-fg' },
  unauthorized: { icon: ShieldAlert, cls: 'text-danger-fg' },
  busy: { icon: Ban, cls: 'text-warning-fg' },
}

export function ConnectionPill({
  state,
  label,
  title,
  className,
}: {
  state: ConnState
  /** The feed.conn.* string for this state (caller supplies; carries the state). */
  label: string
  title?: string
  className?: string
}) {
  const c = CONFIG[state]
  const Icon = c.icon
  return (
    <span
      role="status"
      aria-live="polite"
      title={title}
      className={cn(
        'inline-flex items-center gap-1.5 rounded-pill border border-border bg-subtle px-2 py-0.5 text-caption font-medium',
        c.cls,
        className,
      )}
    >
      <Icon className={cn('h-3.5 w-3.5', c.spin && 'animate-spin')} aria-hidden="true" />
      <span className="text-text-secondary">{label}</span>
    </span>
  )
}
