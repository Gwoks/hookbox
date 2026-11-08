/**
 * The owner nav's building blocks, shared by the desktop TopNav and the
 * mobile "Menu" sheet so both render the same links and actions:
 * section NavLinks (aria-current via react-router), the Share action with
 * its active-link count, and the tint "New rule" shortcut.
 */
import { Plus, Share2 } from 'lucide-react'
import { Link, NavLink } from 'react-router-dom'
import { cn } from '@/lib/cn'
import { t, type CopyKey } from '@/lib/copy'
import { Button } from '@/components/ui/button'
import { Pill } from '@/components/ui/pill'

interface NavItem {
  label: CopyKey
  /** Suffix after `/d/{token}`. */
  path: string
  end?: boolean
}

const NAV_ITEMS: readonly NavItem[] = [
  { label: 'shell.nav.requests', path: '', end: true },
  { label: 'shell.nav.rules', path: '/rules' },
  { label: 'shell.nav.settings', path: '/settings' },
]

export function OwnerNavLinks({
  token,
  vertical = false,
  onNavigate,
}: {
  token: string
  vertical?: boolean
  onNavigate?: () => void
}) {
  return (
    <>
      {NAV_ITEMS.map((item) => (
        <Button key={item.label} variant="nav" size="md" asChild className={cn(vertical && 'justify-start')}>
          <NavLink to={`/d/${token}${item.path}`} end={item.end} onClick={onNavigate}>
            {t(item.label)}
          </NavLink>
        </Button>
      ))}
    </>
  )
}

export function ShareButton({
  count,
  onClick,
  showLabel = false,
  className,
}: {
  count: number | null
  onClick: () => void
  /** Always show the label (the mobile sheet); otherwise it shows from `lg`. */
  showLabel?: boolean
  className?: string
}) {
  const hasLinks = count !== null && count > 0
  return (
    <Button
      variant="ghost"
      size="sm"
      className={className}
      onClick={onClick}
      aria-label={hasLinks ? t('share.action.count.aria', { n: count }) : t('share.action.aria')}
    >
      <Share2 className="h-4 w-4" aria-hidden="true" />
      <span className={cn(!showLabel && 'sr-only lg:not-sr-only')}>{t('share.action')}</span>
      {hasLinks && (
        <Pill tone="neutral" className="tnum">
          {count}
        </Pill>
      )}
    </Button>
  )
}

export function NewRuleLink({
  token,
  onClick,
  className,
}: {
  token: string
  onClick?: () => void
  className?: string
}) {
  return (
    <Button variant="tint" size="sm" asChild className={className}>
      <Link to={`/d/${token}/rules?new=1`} onClick={onClick}>
        <Plus className="h-4 w-4" aria-hidden="true" />
        {t('shell.nav.newRule')}
      </Link>
    </Button>
  )
}
