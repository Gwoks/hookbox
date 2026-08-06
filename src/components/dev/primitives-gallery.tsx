/**
 * Primitives gallery (scaffold issue .26). Exercises every HookBox primitive +
 * reused ui/ primitive in BOTH themes so the design-system foundation is
 * verifiable: token re-theming (AC-D12), grayscale-identifiable chips/selection
 * (AC-D13/D14), focus rings (AC-D15), no-hex (AC-D11 — components only consume
 * tokens). Not a product screen; mounted at /_gallery for QA/Playwright.
 */
import { ThemeToggle } from '@/theme/theme'
import { Button } from '@/components/ui/button'
import { Field, Input, Textarea } from '@/components/ui/input'
import { Switch } from '@/components/ui/switch'
import { Slider } from '@/components/ui/slider'
import { Segmented } from '@/components/ui/segmented'
import { Spinner } from '@/components/ui/spinner'
import { Skeleton } from '@/components/ui/skeleton'
import { useState } from 'react'
import { MethodBadge } from '@/components/hookbox/method-badge'
import { StatusCode } from '@/components/hookbox/status-code'
import { ServedByChip, type ServedBy } from '@/components/hookbox/served-by-chip'
import { ConnectionPill, type ConnState } from '@/components/hookbox/connection-pill'
import { FeedRow } from '@/components/hookbox/feed-row'
import { JsonTree } from '@/components/hookbox/json-tree'
import { CodeBlock, MockUrlChip } from '@/components/hookbox/code-block'
import { KeyValueRows } from '@/components/hookbox/key-value-rows'
import { InlineAlert } from '@/components/hookbox/inline-alert'
import { t } from '@/lib/copy'

const METHODS = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE', 'HEAD', 'OPTIONS']
const STATUSES = [200, 201, 204, 301, 304, 400, 404, 429, 500, 503]
const SERVED: ServedBy[] = ['rule', 'crud', 'mitm', 'tunnel', 'default', 'cors', 'chaos', 'ratelimit']
const CONN: ConnState[] = ['connecting', 'live', 'reconnecting', 'sse', 'offline', 'unauthorized', 'busy']

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <section className="space-y-3 rounded-md border border-border bg-surface p-4">
      <h2 className="text-overline uppercase text-text-tertiary">{title}</h2>
      <div className="flex flex-wrap items-center gap-3">{children}</div>
    </section>
  )
}

export function PrimitivesGallery() {
  const [latency, setLatency] = useState(250)
  const [seg, setSeg] = useState<'pretty' | 'raw'>('pretty')
  const [selected, setSelected] = useState(2)

  return (
    <div className="mx-auto max-w-landing-hero space-y-4 p-6">
      <header className="flex items-center justify-between">
        <h1 className="text-h2 text-text-primary">HookBox primitives</h1>
        <ThemeToggle />
      </header>

      <Section title="Buttons">
        <Button variant="primary">Save</Button>
        <Button variant="secondary">Cancel</Button>
        <Button variant="ghost">Ghost</Button>
        <Button variant="danger">Delete endpoint</Button>
        <Button variant="link">Tunnel from your machine</Button>
        <Button loading>Saving…</Button>
        <Spinner />
      </Section>

      <Section title="MethodBadge (text label is the source of truth)">
        {METHODS.map((m) => (
          <MethodBadge key={m} method={m} />
        ))}
      </Section>

      <Section title="StatusCode (digits + class underline, not hue alone)">
        {STATUSES.map((s) => (
          <StatusCode key={s} code={s} />
        ))}
      </Section>

      <Section title="ServedByChip (icon + text)">
        {SERVED.map((s) => (
          <ServedByChip key={s} servedBy={s} />
        ))}
      </Section>

      <Section title="ConnectionPill (text label carries state)">
        {CONN.map((c) => (
          <ConnectionPill key={c} state={c} label={t(`feed.conn.${c}` as never)} />
        ))}
      </Section>

      <section className="space-y-1 rounded-md border border-border bg-surface p-4">
        <h2 className="text-overline uppercase text-text-tertiary">FeedRow (selected = rail + fill)</h2>
        <div role="listbox" aria-label="Sample feed">
          {[
            { id: 1, method: 'GET', path: '/users/42', status_code: 200, served_by: 'rule' as ServedBy, duration_ms: 12, timestamp: new Date().toISOString() },
            { id: 2, method: 'POST', path: '/orders', status_code: 201, served_by: 'crud' as ServedBy, duration_ms: 8, timestamp: new Date(Date.now() - 60000).toISOString() },
            { id: 3, method: 'DELETE', path: '/sessions/abc', status_code: 500, served_by: 'chaos' as ServedBy, duration_ms: 340, timestamp: new Date(Date.now() - 3600000).toISOString() },
          ].map((r) => (
            <FeedRow key={r.id} row={r} selected={selected === r.id} onSelect={setSelected} />
          ))}
        </div>
      </section>

      <Section title="CodeBlock / MockUrlChip (copy-only, not link-colored)">
        <CodeBlock value="curl https://<your-host>/e/<token>/ping" />
        <MockUrlChip url="https://<your-host>/e/<token>" />
      </Section>

      <div className="grid gap-4 sm:grid-cols-2">
        <section className="space-y-2 rounded-md border border-border bg-surface p-4">
          <h2 className="text-overline uppercase text-text-tertiary">JsonTree</h2>
          <Segmented
            aria-label="view"
            options={[
              { value: 'pretty', label: 'Pretty' },
              { value: 'raw', label: 'Raw' },
            ]}
            value={seg}
            onChange={setSeg}
          />
          <JsonTree raw='{"id":"42","active":true,"roles":["admin","user"],"meta":{"v":2}}' />
        </section>
        <section className="space-y-2 rounded-md border border-border bg-surface p-4">
          <h2 className="text-overline uppercase text-text-tertiary">KeyValueRows</h2>
          <KeyValueRows
            data={{ 'content-type': 'application/json', authorization: '__redacted__', 'x-trace': 'abc' }}
          />
        </section>
      </div>

      <Section title="Form controls">
        <Field label="Endpoint name" helper="Optional. Just a label for you." render={(p) => (
          <Input id={p.id} aria-describedby={p.describedBy} placeholder="e.g. Checkout API" />
        )} />
        <Switch defaultChecked aria-label="Auto-CRUD" />
        <div className="flex items-center gap-2">
          <Slider aria-label="Latency" value={latency} min={0} max={10000} onChange={setLatency} className="w-40" />
          <span className="tnum text-caption text-text-tertiary">{latency}ms</span>
        </div>
        <Textarea placeholder="Response body" mono rows={2} className="w-60" />
      </Section>

      <div className="space-y-3">
        <InlineAlert variant="info" title="Detail on its way">
          This request just landed — its detail is still being written.
        </InlineAlert>
        <InlineAlert variant="warning" title="Stored, not yet sent">
          HookBox saves this with the rule but doesn't fire it yet.
        </InlineAlert>
        <InlineAlert variant="danger" role="alert" title="Couldn't load this request">
          Something went wrong fetching the detail.
        </InlineAlert>
      </div>

      <Section title="Skeleton">
        <Skeleton className="h-8 w-40" />
        <Skeleton className="h-8 w-24" />
      </Section>
    </div>
  )
}
