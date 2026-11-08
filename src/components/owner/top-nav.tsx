/**
 * TopNav — the owner screens' one 64px sticky bar (DESIGN.md: canvas, the
 * nav shadow, page-width row). Left: brand (→ /) and the endpoint switcher.
 * Center (md+): Requests · Rules · Settings with aria-current. Right (md+):
 * Share, the tint "New rule" shortcut (hidden on /rules, whose page header
 * owns the primary), theme, account. Below md everything but the brand and
 * switcher moves into the "Menu" sheet.
 */
import { Link, useLocation } from 'react-router-dom'
import { t } from '@/lib/copy'
import { ThemeToggle } from '@/theme/theme'
import { BrandMark } from '@/components/hookbox/brand-mark'
import { AccountMenu } from './account-menu'
import { useEndpoints } from './endpoint-context'
import { EndpointSwitcher } from './endpoint-switcher'
import { MobileMenu } from './mobile-menu'
import { NewRuleLink, OwnerNavLinks, ShareButton } from './nav-items'

export interface TopNavProps {
  shareCount: number | null
  onShare: () => void
  onSignOut: () => void
}

export function TopNav({ shareCount, onShare, onSignOut }: TopNavProps) {
  const { token } = useEndpoints()
  const { pathname } = useLocation()
  const showNewRule = !pathname.startsWith(`/d/${token}/rules`)

  return (
    <header className="sticky top-0 z-nav h-nav bg-canvas shadow-nav">
      <div className="mx-auto flex h-full max-w-page items-center gap-3 px-4 sm:px-6">
        <div className="flex min-w-0 flex-1 items-center gap-3">
          <Link to="/" className="shrink-0 rounded-sm">
            <BrandMark wordmark="lg" />
          </Link>
          <EndpointSwitcher />
        </div>
        <nav aria-label={t('layout.nav.aria')} className="hidden items-center gap-1 md:flex">
          <OwnerNavLinks token={token} />
        </nav>
        <div className="flex flex-1 items-center justify-end gap-1.5">
          <div className="hidden items-center gap-1.5 md:flex">
            <ShareButton count={shareCount} onClick={onShare} />
            {showNewRule && <NewRuleLink token={token} />}
            <ThemeToggle />
            <AccountMenu token={token} onSignOut={onSignOut} />
          </div>
          <MobileMenu
            token={token}
            shareCount={shareCount}
            showNewRule={showNewRule}
            onShare={onShare}
            onSignOut={onSignOut}
            triggerClassName="md:hidden"
          />
        </div>
      </div>
    </header>
  )
}
