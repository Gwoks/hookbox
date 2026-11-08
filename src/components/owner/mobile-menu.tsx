/**
 * MobileMenu — below `md` the owner nav collapses into an "Open menu" button
 * and a right Sheet titled "Menu" (design call D5): section links, New rule,
 * Share, theme and the account items. Actions that open another dialog
 * (Share, Sign out) run only after the sheet has closed, so Radix's focus
 * return to "Open menu" never fights the next dialog's focus trap.
 */
import { Menu as MenuIcon } from 'lucide-react'
import { useRef, useState } from 'react'
import { Link } from 'react-router-dom'
import { t } from '@/lib/copy'
import { ThemeToggle } from '@/theme/theme'
import { Button } from '@/components/ui/button'
import { Overline } from '@/components/ui/overline'
import { Sheet, SheetContent, SheetTrigger } from '@/components/ui/sheet'
import { OwnerIdPreview, recoveryKeyPath } from './account-menu'
import { NewRuleLink, OwnerNavLinks, ShareButton } from './nav-items'

export interface MobileMenuProps {
  token: string
  shareCount: number | null
  showNewRule: boolean
  onShare: () => void
  onSignOut: () => void
  triggerClassName?: string
}

const ROW = 'w-full justify-start'

export function MobileMenu({ token, shareCount, showNewRule, onShare, onSignOut, triggerClassName }: MobileMenuProps) {
  const [open, setOpen] = useState(false)
  const afterClose = useRef<(() => void) | null>(null)

  function closeThen(action?: () => void) {
    afterClose.current = action ?? null
    setOpen(false)
  }

  function handleCloseAutoFocus(event: Event) {
    const action = afterClose.current
    afterClose.current = null
    if (!action) return
    event.preventDefault()
    action()
  }

  return (
    <Sheet open={open} onOpenChange={setOpen}>
      <SheetTrigger asChild>
        <Button variant="ghost" size="icon" className={triggerClassName} aria-label={t('shell.mobileNav.open.aria')}>
          <MenuIcon className="h-5 w-5" aria-hidden="true" />
        </Button>
      </SheetTrigger>
      <SheetContent side="right" title={t('shell.mobileNav.title')} onCloseAutoFocus={handleCloseAutoFocus}>
        <nav aria-label={t('layout.nav.aria')} className="flex flex-col gap-1">
          <OwnerNavLinks token={token} vertical onNavigate={() => closeThen()} />
        </nav>
        <div className="my-4 flex flex-col gap-2 border-t border-border pt-4">
          {showNewRule && <NewRuleLink token={token} className={ROW} onClick={() => closeThen()} />}
          <ShareButton count={shareCount} showLabel className={ROW} onClick={() => closeThen(onShare)} />
          <ThemeToggle showLabel className={ROW} />
        </div>
        <div className="flex flex-col gap-1 border-t border-border pt-4">
          <Overline as="h3" className="px-2 pb-1">
            {t('shell.account.label')}
          </Overline>
          <OwnerIdPreview className="px-2 pb-2 text-caption text-text-tertiary" />
          <Button variant="ghost" size="sm" asChild className={ROW}>
            <Link to={recoveryKeyPath(token)} onClick={() => closeThen()}>
              {t('shell.account.recoveryKey')}
            </Link>
          </Button>
          <Button variant="ghost" size="sm" className={ROW} onClick={() => closeThen(onSignOut)}>
            {t('shell.account.signOut')}
          </Button>
        </div>
      </SheetContent>
    </Sheet>
  )
}
