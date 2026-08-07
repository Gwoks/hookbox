/** ConfirmDialog (design.md §3.2/§9.1, AC-83). Generic two-button destructive
 * confirm: DialogHeader title, DialogBody text, ghost Cancel + `variant="danger"`
 * confirm. Extracted from a private helper in settings.tsx whose
 * `try { await onConfirm(); onClose() } finally {}` had NO `catch` — a
 * rejection escaped as an unhandled promise rejection and the user was told
 * nothing. This version catches, keeps the dialog open, renders the failure
 * inline (an `InlineAlert variant="danger" role="alert"`), and disables
 * Cancel while a confirm is in flight. Every new destructive confirm in this
 * batch (F1's Clear all, F3's import-failure recovery, F4's revoke) is meant
 * to share this instead of re-implementing it. */
import { useState } from 'react'
import { ApiError } from '@/api'
import { t } from '@/lib/copy'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogBody,
  DialogContent,
  DialogFooter,
  DialogHeader,
} from '@/components/ui/dialog'
import { InlineAlert } from './inline-alert'

function messageFor(err: unknown, fallback?: string): string {
  if (err instanceof ApiError && err.message) return err.message
  return fallback ?? t('common.error.generic')
}

export function ConfirmDialog({
  open,
  onClose,
  title,
  body,
  confirmLabel,
  cancelLabel,
  errorFallback,
  onConfirm,
  confirmVariant = 'danger',
}: {
  open: boolean
  onClose: () => void
  title: React.ReactNode
  body: React.ReactNode
  confirmLabel: string
  /** Defaults to the shared `common.cancel` string. */
  cancelLabel?: string
  /** Shown when the rejection carries no server `detail`. */
  errorFallback?: string
  onConfirm: () => Promise<void>
  /** Most confirms are destructive (danger); F6's shadow confirm is
   * recoverable, so it opts into `primary` instead. */
  confirmVariant?: 'danger' | 'primary'
}) {
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)

  function handleOpenChange(nextOpen: boolean) {
    if (nextOpen || busy) return
    setError(null)
    onClose()
  }

  async function handleConfirm() {
    setBusy(true)
    setError(null)
    try {
      await onConfirm()
      setBusy(false)
      onClose()
    } catch (err) {
      setBusy(false)
      setError(messageFor(err, errorFallback))
    }
  }

  return (
    <Dialog open={open} onOpenChange={handleOpenChange}>
      <DialogContent>
        <DialogHeader>{title}</DialogHeader>
        <DialogBody>
          <div className="text-body-sm text-text-secondary">{body}</div>
          {error && (
            <InlineAlert variant="danger" role="alert" className="mt-3">
              {error}
            </InlineAlert>
          )}
        </DialogBody>
        <DialogFooter>
          <Button variant="ghost" onClick={() => handleOpenChange(false)} disabled={busy}>
            {cancelLabel ?? t('common.cancel')}
          </Button>
          <Button variant={confirmVariant} loading={busy} onClick={() => void handleConfirm()}>
            {confirmLabel}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
