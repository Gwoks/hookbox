/**
 * The landing's call to action (hookbox-a73.3): the one primary — "Get my
 * endpoint" mints an anonymous owner + primary endpoint and opens it with
 * `{ fresh: true }` (the dashboard then nudges to save the key) — and the
 * tint "I have a recovery key", which reveals the restore form. Banners:
 * the 401-bounce reason, mint / resume failures, blocked cookies.
 */
import { useState } from 'react'
import { useLocation, useNavigate } from 'react-router-dom'
import { api, resolvePrimaryToken, session } from '@/api'
import { signInErrorMessage } from '@/lib/api-errors'
import { t } from '@/lib/copy'
import { useAsyncAction } from '@/lib/hooks/use-async-action'
import { InlineAlert } from '@/components/hookbox/inline-alert'
import { RestoreKeyForm } from '@/components/owner/restore-key-form'
import { Button } from '@/components/ui/button'
import { Card } from '@/components/ui/card'

export const GET_STARTED_ID = 'get-started'

export function GetStarted({ hasResumeFailed, areCookiesEnabled }: { hasResumeFailed: boolean; areCookiesEnabled: boolean }) {
  const navigate = useNavigate()
  const location = useLocation()
  const [isRestoreOpen, setIsRestoreOpen] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const start = useAsyncAction(async () => {
    const summary = await api.createSession()
    const token = await resolvePrimaryToken(summary)
    return { ownerId: summary.owner_id, token }
  })
  const bounceReason = (location.state as { reason?: unknown } | null)?.reason

  async function handleStart() {
    setError(null)
    const result = await start.run()
    if (!result.ok) {
      setError(signInErrorMessage(result.error))
      return
    }
    session.markSignedIn(result.value.ownerId, { broadcast: true })
    navigate(`/d/${result.value.token}`, { replace: true, state: { fresh: true } })
  }

  return (
    <div id={GET_STARTED_ID} className="mx-auto w-full max-w-form scroll-mt-nav space-y-4">
      {typeof bounceReason === 'string' && (
        <InlineAlert variant="info" role="status">
          {bounceReason}
        </InlineAlert>
      )}
      {(error || hasResumeFailed) && (
        <InlineAlert variant="danger" role="alert">
          {error ?? t('landing.error.network')}
        </InlineAlert>
      )}
      {!areCookiesEnabled && (
        <InlineAlert variant="warning" role="status">
          {t('landing.warn.cookies')}
        </InlineAlert>
      )}
      <div className="flex flex-wrap justify-center gap-3">
        <Button variant="primary" size="lg" loading={start.pending} onClick={() => void handleStart()}>
          {start.pending ? t('landing.start.submitting') : t('landing.start.submit')}
        </Button>
        <Button variant="tint" size="lg" aria-expanded={isRestoreOpen} onClick={() => setIsRestoreOpen((open) => !open)}>
          {t('landing.restore.toggle')}
        </Button>
      </div>
      <p className="text-center text-body-sm text-text-tertiary">{t('landing.start.helper')}</p>
      {isRestoreOpen && (
        <Card className="text-left">
          <RestoreKeyForm />
        </Card>
      )}
    </div>
  )
}
