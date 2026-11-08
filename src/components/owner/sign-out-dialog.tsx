/**
 * Sign-out confirm (hookbox-a73.3). Signing out clears the cookie from this
 * browser; with no email on file, an owner who has not saved their recovery
 * key loses these endpoints for good — so the dialog says so and offers a
 * one-click "Copy recovery key" first.
 *
 * Confirm → DELETE /api/session → mark this tab (and, via the broadcast,
 * every other tab of this browser) signed out → land on /.
 */
import { useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { Check, Copy } from 'lucide-react'
import { api, session } from '@/api'
import { errorMessage } from '@/lib/api-errors'
import { t } from '@/lib/copy'
import { useAsyncAction } from '@/lib/hooks/use-async-action'
import { ConfirmDialog } from '@/components/hookbox/confirm-dialog'
import { Button } from '@/components/ui/button'

type CopyState = { kind: 'idle' } | { kind: 'copied' } | { kind: 'manual'; key: string }

/** Fetch the key and put it on the clipboard; when the clipboard is blocked
 * (plain-HTTP deployments), show it for manual selection instead. */
function CopyKeyAction() {
  const [state, setState] = useState<CopyState>({ kind: 'idle' })
  const reveal = useAsyncAction(() => api.revealKey())

  async function copyKey() {
    const result = await reveal.run()
    if (!result.ok) return
    try {
      await navigator.clipboard.writeText(result.value.key)
      setState({ kind: 'copied' })
    } catch {
      setState({ kind: 'manual', key: result.value.key })
    }
  }

  const isCopied = state.kind === 'copied'
  return (
    <div className="space-y-2">
      <Button variant="outline" size="sm" loading={reveal.pending} onClick={() => void copyKey()}>
        {isCopied ? <Check className="h-3.5 w-3.5" aria-hidden="true" /> : <Copy className="h-3.5 w-3.5" aria-hidden="true" />}
        {t('shell.account.signOut.copyKey')}
      </Button>
      <p role="status" className="text-caption text-text-tertiary">
        {isCopied ? t('common.copy.announce') : ''}
      </p>
      {state.kind === 'manual' && (
        <div className="space-y-1">
          <p className="text-caption text-text-secondary">{t('shell.account.signOut.copyKey.fallback')}</p>
          <code className="block break-all rounded-sm border border-border bg-surface-subtle px-2 py-1.5 font-mono text-mono-sm text-text-primary">
            {state.key}
          </code>
        </div>
      )}
      {reveal.error !== null && (
        <p role="alert" className="text-caption text-danger-fg">
          {errorMessage(reveal.error, t('set.recovery.error.reveal'))}
        </p>
      )}
    </div>
  )
}

export function SignOutDialog({ open, onClose }: { open: boolean; onClose: () => void }) {
  const navigate = useNavigate()

  async function signOut() {
    await api.signOut()
    session.markSignedOut({ broadcast: true })
    navigate('/', { replace: true })
  }

  return (
    <ConfirmDialog
      open={open}
      onClose={onClose}
      title={t('shell.account.signOut.confirmTitle')}
      body={
        <div className="space-y-3">
          <p>{t('shell.account.signOut.confirmBody')}</p>
          <CopyKeyAction />
        </div>
      }
      confirmLabel={t('shell.account.signOut.confirm')}
      errorFallback={t('shell.account.signOut.error')}
      onConfirm={signOut}
    />
  )
}
