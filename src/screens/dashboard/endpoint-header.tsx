/**
 * The dashboard's compact page header: the endpoint name (or its token, in
 * mono) as the h1, then one meta row — the copy-only Mock URL chip (AC-D19;
 * the label "Mock URL" appears exactly once on the dashboard, AC-7), the
 * Auto-CRUD glance, the live "Tunnel connected" reflection and the feed's
 * connection pill.
 */
import { Database, RadioTower } from 'lucide-react'
import type { ReactNode } from 'react'
import type { EndpointDetail } from '@/api'
import { t } from '@/lib/copy'
import { absolutize } from '@/lib/url'
import { MockUrlChip } from '@/components/hookbox/code-block'
import { Overline } from '@/components/ui/overline'
import { PageHeader } from '@/components/ui/page-header'
import { Pill } from '@/components/ui/pill'
import { Tooltip } from '@/components/ui/tooltip'

export function EndpointHeader({ endpoint, connection }: { endpoint: EndpointDetail; connection: ReactNode }) {
  const title = endpoint.name || <span className="font-mono">{endpoint.token}</span>
  return (
    <PageHeader
      size="compact"
      title={title}
      meta={
        <>
          <Tooltip content={t('dash.mockUrl.tooltip')}>
            <span className="flex min-w-0 max-w-full flex-wrap items-center gap-2">
              <Overline as="span">{t('dash.mockUrl.label')}</Overline>
              <MockUrlChip url={absolutize(endpoint.mock_url)} className="min-w-0 max-w-full" />
            </span>
          </Tooltip>
          {endpoint.auto_crud && (
            <Tooltip content={t('dash.autoCrud.tooltip')}>
              <span className="inline-flex">
                <Pill tone="neutral">
                  <Database className="h-3 w-3" aria-hidden="true" />
                  {t('dash.autoCrud.label')}
                </Pill>
              </span>
            </Tooltip>
          )}
          {endpoint.tunnel_active && (
            <Tooltip content={t('dash.tunnel.active.tooltip')}>
              <span className="inline-flex">
                <Pill tone="none" className="bg-served-tunnel-bg px-2 py-0.5 text-caption text-served-tunnel-fg">
                  <RadioTower className="h-3 w-3" aria-hidden="true" />
                  {t('dash.tunnel.active')}
                </Pill>
              </span>
            </Tooltip>
          )}
          {connection}
        </>
      }
    />
  )
}
