/**
 * AccountMenu — the anonymous owner's menu: a short owner-id preview (never
 * an email, never the key), a jump to the recovery-key card, and sign out
 * (confirmed by the caller's SignOutDialog). `OwnerIdPreview` is reused by
 * the mobile sheet.
 */
import { CircleUser } from 'lucide-react'
import { useNavigate } from 'react-router-dom'
import { useSession } from '@/api'
import { t } from '@/lib/copy'
import { Button } from '@/components/ui/button'
import { Menu, MenuContent, MenuItem, MenuSeparator, MenuTrigger } from '@/components/ui/menu'
import { RECOVERY_KEY_SECTION_ID } from './recovery-key-section'

/** The account menu shows only this much of the anonymous owner id. */
const OWNER_ID_PREVIEW_LENGTH = 8

export function recoveryKeyPath(token: string): string {
  return `/d/${token}/settings#${RECOVERY_KEY_SECTION_ID}`
}

export function OwnerIdPreview({ className }: { className?: string }) {
  const { ownerId } = useSession()
  if (!ownerId) return null
  return (
    <p className={className ?? 'px-2 py-1.5 text-caption text-text-tertiary'}>
      {t('shell.account.owner', { id: ownerId.slice(0, OWNER_ID_PREVIEW_LENGTH) })}
    </p>
  )
}

export function AccountMenu({ token, onSignOut }: { token: string; onSignOut: () => void }) {
  const navigate = useNavigate()
  const { ownerId } = useSession()
  return (
    <Menu>
      <MenuTrigger asChild>
        <Button variant="ghost" size="sm" aria-label={t('shell.account.label')}>
          <CircleUser className="h-4 w-4" aria-hidden="true" />
          <span className="hidden lg:inline">{t('shell.account.label')}</span>
        </Button>
      </MenuTrigger>
      <MenuContent align="end">
        {ownerId && (
          <>
            <OwnerIdPreview />
            <MenuSeparator />
          </>
        )}
        <MenuItem onSelect={() => navigate(recoveryKeyPath(token))}>{t('shell.account.recoveryKey')}</MenuItem>
        <MenuItem onSelect={onSignOut}>{t('shell.account.signOut')}</MenuItem>
      </MenuContent>
    </Menu>
  )
}
