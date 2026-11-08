/**
 * The public /s/:code viewer (operator-toolkit F4, PRD §3/§4.4, design.md
 * §3.7). Anyone holding the URL — no account, no sign-in, no owner key,
 * and this screen never creates one (AC-41/AC-42/AC-S13). Its module graph
 * must stay on the public client and the viewer-safe component folders
 * (e2e/viewer-import-graph + contract.spec enforce that), it renders zero
 * accent-filled controls (AC-109), and its chrome is the public header with
 * a brand that is not a link — no owner nav, no actions.
 */
import { useEffect } from 'react'
import { Link, useParams } from 'react-router-dom'
import { t } from '@/lib/copy'
import { useCountdown } from '@/lib/hooks/use-countdown'
import { InlineAlert } from '@/components/hookbox/inline-alert'
import { RetryAlert } from '@/components/hookbox/retry-alert'
import { FullPageMessage } from '@/components/layout/full-page-message'
import { PublicPage } from '@/components/layout/public-page'
import { Button } from '@/components/ui/button'
import { PageHeader } from '@/components/ui/page-header'
import { RequestsCard } from './share-view/requests-card'
import { useSharedFeed } from './share-view/use-shared-feed'

const footer = <p>{t('viewer.footer')}</p>

function useViewerTitle() {
  useEffect(() => {
    document.title = t('viewer.docTitle')
  }, [])
}

/** AC-106's terminal state — no banner, no list; header + footer stay. */
function UnavailablePage() {
  useViewerTitle()
  return (
    <PublicPage width="content" footer={footer} mainClassName="flex flex-col">
      <FullPageMessage
        title={t('viewer.unavailable.title')}
        description={t('viewer.unavailable.body')}
        actions={
          <Button variant="link" asChild>
            <Link to="/">{t('viewer.unavailable.about')}</Link>
          </Button>
        }
      />
    </PublicPage>
  )
}

/** AC-107: a standing, non-dismissible notice spanning the page. */
const banner = (
  <div className="border-b border-border bg-surface">
    <div className="mx-auto w-full max-w-content px-4 py-3 sm:px-6">
      <InlineAlert variant="info" role="status" title={t('viewer.banner.title')}>
        {t('viewer.banner.body')}
      </InlineAlert>
    </div>
  </div>
)

export function ShareView() {
  const { code = '' } = useParams()
  const { status, data, retryInSeconds, refresh, isHidden, isOffline } = useSharedFeed(code)
  const countdown = useCountdown(status === 'rateLimited' ? retryInSeconds : null)
  useViewerTitle()

  if (status === 'unavailable') return <UnavailablePage />

  const subject = data && (
    <span className="block truncate">
      <span title={data.endpoint.name ?? undefined}>
        {t('viewer.subject.name', { name: data.endpoint.name || t('viewer.subject.unnamed') })}
      </span>
      {' · '}
      {t('viewer.subject.total', { n: data.endpoint.request_count })}
    </span>
  )

  return (
    <PublicPage width="content" banner={banner} footer={footer} mainClassName="space-y-5">
      {/* AC-107: the h1 is ALWAYS the static string, never the endpoint name. */}
      <PageHeader size="doc" title={t('viewer.title')} description={subject} />

      {isOffline && (
        <RetryAlert variant="info" role="status" title={t('viewer.offline.title')}>
          {t('viewer.offline.body')}
        </RetryAlert>
      )}
      {!isOffline && status === 'rateLimited' && (
        <RetryAlert variant="warning" role="status" title={t('viewer.rateLimited.title')}>
          {t('viewer.rateLimited.body', { seconds: countdown ?? retryInSeconds ?? 0 })}
        </RetryAlert>
      )}
      {!isOffline && status === 'error' && (
        <RetryAlert title={t('viewer.error.title')} onRetry={refresh}>
          {t('viewer.error.body')}
        </RetryAlert>
      )}

      <RequestsCard code={code} data={data} status={status} onRefresh={refresh} />

      <p className="text-caption text-text-tertiary">{isHidden ? t('viewer.updating.paused') : t('viewer.updating')}</p>
    </PublicPage>
  )
}
