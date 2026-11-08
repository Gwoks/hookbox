/**
 * The landing's product mockup — DESIGN.md allows its single drop shadow.
 * A static picture of the dashboard built from the real request
 * primitives: a mini top bar, five feed rows (RequestCells) and an
 * inspector card. It is `inert` and aria-hidden (nothing in it works), with
 * an sr-only caption describing it instead.
 */
import type { RequestCellsData } from '@/components/hookbox/request-cells'
import { REQUEST_GRID, RequestCells } from '@/components/hookbox/request-cells'
import { RequestSubject } from '@/components/hookbox/request-subject'
import { cn } from '@/lib/cn'
import { t } from '@/lib/copy'
import { Card } from '@/components/ui/card'
import { Overline } from '@/components/ui/overline'

const SECOND_MS = 1000
const ago = (seconds: number) => new Date(Date.now() - seconds * SECOND_MS).toISOString()

const ROWS: readonly (RequestCellsData & { id: number })[] = [
  { id: 5, method: 'POST', path: '/webhooks/stripe', status_code: 200, served_by: 'rule', duration_ms: 3, timestamp: ago(2) },
  { id: 4, method: 'GET', path: '/orders/42', status_code: 200, served_by: 'crud', duration_ms: 2, timestamp: ago(9) },
  { id: 3, method: 'POST', path: '/login', status_code: 201, served_by: 'rule', duration_ms: 4, timestamp: ago(21) },
  { id: 2, method: 'GET', path: '/users/me', status_code: 200, served_by: 'mitm', duration_ms: 88, timestamp: ago(47) },
  { id: 1, method: 'DELETE', path: '/orders/7', status_code: 500, served_by: 'chaos', duration_ms: 1, timestamp: ago(63) },
]

const HEADERS: readonly [string, string][] = [
  ['content-type', 'application/json'],
  ['stripe-signature', 't=1731024000,v1=5257a8…'],
  ['user-agent', 'Stripe/1.0'],
]

/** React 18's types predate the `inert` attribute. */
const INERT = { inert: '' } as Record<string, string>

export function ProductMockup() {
  return (
    <figure className="mx-auto w-full max-w-wide">
      <figcaption className="sr-only">{t('landing.mockup.caption')}</figcaption>
      <Card padding="none" className="overflow-hidden shadow-mockup" aria-hidden="true" {...INERT}>
        <div className="flex items-center gap-3 border-b border-border px-4 py-3">
          <span className="h-2.5 w-2.5 rounded-pill bg-accent" />
          <span className="rounded-xs border border-border px-2 py-0.5 font-mono text-mono-sm text-text-secondary">
            {t('landing.mockup.endpoint')}
          </span>
          <span className="ml-auto text-caption text-text-tertiary">{t('feed.conn.live')}</span>
        </div>
        <div className="grid md:grid-cols-[minmax(0,1.25fr)_minmax(0,1fr)]">
          <div className="min-w-0 border-border md:border-r">
            <Overline className="px-4 pb-2 pt-3">{t('feed.header.title')}</Overline>
            {ROWS.map((row, i) => (
              <div
                key={row.id}
                className={cn(
                  'grid items-center gap-2 border-t border-border px-4 py-2 text-body-sm',
                  REQUEST_GRID,
                  i === 0 && 'bg-surface-active shadow-[inset_3px_0_0_var(--accent)]',
                )}
              >
                <RequestCells request={row} />
              </div>
            ))}
          </div>
          <div className="hidden min-w-0 space-y-3 p-4 md:block">
            <RequestSubject request={ROWS[0]} />
            <Overline>{t('insp.tab.headers')}</Overline>
            <dl className="space-y-1.5">
              {HEADERS.map(([key, value]) => (
                <div key={key} className="grid grid-cols-[8rem_minmax(0,1fr)] gap-2 font-mono text-mono-sm">
                  <dt className="truncate text-text-secondary">{key}</dt>
                  <dd className="truncate text-text-primary">{value}</dd>
                </div>
              ))}
            </dl>
          </div>
        </div>
      </Card>
    </figure>
  )
}
