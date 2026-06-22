/** ServedByChip (design.md §3.4 / §2.3, AC-56/D14). radius-xs chip, lucide icon
 * + text, fg/bg from the served-by token set. Same chip in FeedRow + inspector
 * subject strip — "one subject, one color." Every chip = icon + text so hue is
 * secondary (grayscale-identifiable). `echo` is the normal default chip — no
 * special label (copy C5 / AC-J10). */
import {
  ArrowLeftRight,
  CircleDashed,
  Database,
  Gauge,
  GitBranch,
  RadioTower,
  ShieldCheck,
  Zap,
  type LucideIcon,
} from 'lucide-react'
import { cn } from '@/lib/cn'

export type ServedBy =
  | 'rule'
  | 'crud'
  | 'mitm'
  | 'tunnel'
  | 'default'
  | 'cors'
  | 'chaos'
  | 'ratelimit'

const CONFIG: Record<ServedBy, { icon: LucideIcon; cls: string }> = {
  rule: { icon: GitBranch, cls: 'text-served-rule-fg bg-served-rule-bg' },
  crud: { icon: Database, cls: 'text-served-crud-fg bg-served-crud-bg' },
  mitm: { icon: ArrowLeftRight, cls: 'text-served-mitm-fg bg-served-mitm-bg' },
  tunnel: { icon: RadioTower, cls: 'text-served-tunnel-fg bg-served-tunnel-bg' },
  default: { icon: CircleDashed, cls: 'text-served-default-fg bg-served-default-bg' },
  cors: { icon: ShieldCheck, cls: 'text-served-cors-fg bg-served-cors-bg' },
  chaos: { icon: Zap, cls: 'text-served-chaos-fg bg-served-chaos-bg' },
  ratelimit: { icon: Gauge, cls: 'text-served-ratelimit-fg bg-served-ratelimit-bg' },
}

export function ServedByChip({ servedBy, className }: { servedBy: ServedBy; className?: string }) {
  const c = CONFIG[servedBy] ?? CONFIG.default
  const Icon = c.icon
  return (
    <span
      className={cn(
        'inline-flex items-center gap-1 rounded-xs px-1.5 py-0.5 text-caption font-medium',
        c.cls,
        className,
      )}
    >
      <Icon className="h-3 w-3" aria-hidden="true" />
      {servedBy}
    </span>
  )
}
