/**
 * "Save your recovery key" nudge (hookbox-a73.3). Shown on the dashboard only
 * right after the landing minted a brand-new anonymous owner (router state
 * `{ fresh: true }`): with no email, that key is the only way back. Dismiss
 * clears the router state so a reload does not bring it back.
 */
import { Link, useLocation, useNavigate } from 'react-router-dom'
import { t } from '@/lib/copy'
import { InlineAlert } from '@/components/hookbox/inline-alert'
import { RECOVERY_KEY_SECTION_ID } from '@/components/owner/recovery-key-section'
import { Button } from '@/components/ui/button'

export function SaveKeyAlert({ token }: { token: string }) {
  const location = useLocation()
  const navigate = useNavigate()
  const isFresh = (location.state as { fresh?: unknown } | null)?.fresh === true
  if (!isFresh) return null

  function dismiss() {
    navigate({ pathname: location.pathname, search: location.search }, { replace: true, state: null })
  }

  return (
    <div className="border-b border-border px-4 py-2">
      <InlineAlert
        variant="warning"
        role="status"
        title={t('dash.saveKey.title')}
        action={
          <div className="flex flex-wrap items-center gap-2">
            <Button variant="outline" size="sm" asChild>
              <Link to={`/d/${token}/settings#${RECOVERY_KEY_SECTION_ID}`}>{t('dash.saveKey.action')}</Link>
            </Button>
            <Button variant="ghost" size="sm" onClick={dismiss}>
              {t('common.dismiss')}
            </Button>
          </div>
        }
      >
        {t('dash.saveKey.body')}
      </InlineAlert>
    </div>
  )
}
