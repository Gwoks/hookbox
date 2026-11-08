/**
 * RecoveryKeySection — Settings' "Recovery key" card (`#recovery-key`,
 * hookbox-a73.3). The anonymous key is the ONLY way back to an owner's
 * endpoints, so this is where the owner saves, replaces or swaps it:
 *
 * - Reveal / Hide + Copy: POST /api/session/key. The key lives only in this
 *   component's state while revealed — never in storage, never in a URL.
 * - Rotate: behind a ConfirmDialog (it signs other browsers out and stops a
 *   running tunnel CLI); POST /api/session/rotate returns the new key, shown
 *   revealed so it can be saved at once. This browser gets the new cookie.
 * - Use a different recovery key: the shared RestoreKeyForm.
 */
import { useEffect, useState } from 'react'
import { useLocation } from 'react-router-dom'
import { api } from '@/api'
import { errorMessage } from '@/lib/api-errors'
import { t } from '@/lib/copy'
import { useAsyncAction } from '@/lib/hooks/use-async-action'
import { ConfirmDialog } from '@/components/hookbox/confirm-dialog'
import { InlineAlert } from '@/components/hookbox/inline-alert'
import { Button } from '@/components/ui/button'
import { CopyButton } from '@/components/ui/copy-button'
import { SettingsSection } from '@/components/ui/section'
import { RestoreKeyForm } from './restore-key-form'

export const RECOVERY_KEY_SECTION_ID = 'recovery-key'
const MASKED_KEY = '•'.repeat(24)

export function RecoveryKeySection() {
  const location = useLocation()
  const [revealedKey, setRevealedKey] = useState<string | null>(null)
  const [rotatedNotice, setRotatedNotice] = useState(false)
  const [rotateOpen, setRotateOpen] = useState(false)
  const [restoreOpen, setRestoreOpen] = useState(false)
  const reveal = useAsyncAction(() => api.revealKey())

  // `settings#recovery-key` links (account menu, the dashboard's "save your
  // key" alert) land here; the router does not scroll to hashes itself.
  useEffect(() => {
    if (location.hash !== `#${RECOVERY_KEY_SECTION_ID}`) return
    document.getElementById(RECOVERY_KEY_SECTION_ID)?.scrollIntoView({ block: 'start' })
  }, [location.hash])

  async function toggleReveal() {
    if (revealedKey) {
      setRevealedKey(null)
      setRotatedNotice(false)
      return
    }
    const result = await reveal.run()
    if (result.ok) setRevealedKey(result.value.key)
  }

  async function rotate() {
    const { key } = await api.rotateKey()
    reveal.reset()
    setRevealedKey(key)
    setRotatedNotice(true)
  }

  return (
    <SettingsSection
      id={RECOVERY_KEY_SECTION_ID}
      title={t('set.recovery.title')}
      description={t('set.recovery.description')}
    >
      <div className="flex flex-wrap items-center gap-2">
        <code className="min-w-0 flex-1 break-all rounded-sm border border-border bg-surface-subtle px-3 py-2 font-mono text-mono-sm text-text-primary">
          {revealedKey ?? (
            <>
              <span aria-hidden="true">{MASKED_KEY}</span>
              <span className="sr-only">{t('set.recovery.hidden.aria')}</span>
            </>
          )}
        </code>
        <Button
          variant="outline"
          aria-pressed={revealedKey !== null}
          loading={reveal.pending}
          onClick={() => void toggleReveal()}
        >
          {revealedKey ? t('set.recovery.hide') : t('set.recovery.reveal')}
        </Button>
        {revealedKey && <CopyButton value={revealedKey} label={t('set.recovery.copy')} size="sm" />}
      </div>

      {reveal.error !== null && (
        <InlineAlert variant="danger" role="alert">
          {errorMessage(reveal.error, t('set.recovery.error.reveal'))}
        </InlineAlert>
      )}
      {rotatedNotice && (
        <InlineAlert variant="info" role="status">
          {t('set.recovery.rotate.done')}
        </InlineAlert>
      )}
      {revealedKey && <p className="text-caption text-text-tertiary">{t('set.recovery.warning')}</p>}

      <div className="flex flex-wrap items-center justify-between gap-3 border-t border-border pt-4">
        <p className="text-body-sm text-text-secondary">{t('set.recovery.rotate.helper')}</p>
        <Button variant="danger-ghost" onClick={() => setRotateOpen(true)}>
          {t('set.recovery.rotate')}
        </Button>
      </div>

      <div className="space-y-3 border-t border-border pt-4">
        <Button variant="ghost" aria-expanded={restoreOpen} onClick={() => setRestoreOpen((open) => !open)}>
          {t('set.recovery.restore.toggle')}
        </Button>
        {restoreOpen && <RestoreKeyForm helper={t('set.recovery.restore.helper')} />}
      </div>

      <ConfirmDialog
        open={rotateOpen}
        onClose={() => setRotateOpen(false)}
        title={t('set.recovery.rotate.confirmTitle')}
        body={t('set.recovery.rotate.confirmBody')}
        confirmLabel={t('set.recovery.rotate.confirm')}
        errorFallback={t('set.recovery.rotate.error')}
        onConfirm={rotate}
      />
    </SettingsSection>
  )
}
