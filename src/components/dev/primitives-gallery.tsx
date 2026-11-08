/**
 * Primitives gallery (/_gallery, dev-only — router.tsx drops the route from
 * production builds). Renders every primitive, variant and tone so token
 * re-theming can be eyeballed in BOTH themes (toggle top-right): buttons,
 * the field system, cards and pills, page chrome, request primitives, alerts
 * and loading/empty states. Not a product screen.
 */
import { useState, type ReactNode } from 'react'
import { Inbox } from 'lucide-react'
import type { PublicRequestDetail } from '@/api/schemas'
import { t } from '@/lib/copy'
import { Button } from '@/components/ui/button'
import { Card, CardHeader, type CardTone } from '@/components/ui/card'
import { EmptyState } from '@/components/ui/empty-state'
import { Fieldset } from '@/components/ui/field'
import { Field, Input, TextField, Textarea } from '@/components/ui/input'
import { ListEditor } from '@/components/ui/list-editor'
import { LoadingBlock } from '@/components/ui/loading-block'
import { NumberSlider } from '@/components/ui/number-slider'
import { Overline } from '@/components/ui/overline'
import { PageHeader } from '@/components/ui/page-header'
import { Pill, type PillProps } from '@/components/ui/pill'
import { SettingsSection } from '@/components/ui/section'
import { Segmented } from '@/components/ui/segmented'
import { SelectField } from '@/components/ui/select'
import { Sheet, SheetContent, SheetTrigger } from '@/components/ui/sheet'
import { Spinner } from '@/components/ui/spinner'
import { SwitchField } from '@/components/ui/switch'
import { CodeBlock, MockUrlChip } from '@/components/hookbox/code-block'
import { ConnectionPill, type ConnState } from '@/components/hookbox/connection-pill'
import { FeedRow } from '@/components/hookbox/feed-row'
import { InlineAlert } from '@/components/hookbox/inline-alert'
import { JsonTree } from '@/components/hookbox/json-tree'
import { MethodBadge } from '@/components/hookbox/method-badge'
import { REQUEST_GRID_DISCLOSURE, RequestCells } from '@/components/hookbox/request-cells'
import { RequestDetailTabs } from '@/components/hookbox/request-detail-tabs'
import { RequestSubject } from '@/components/hookbox/request-subject'
import { RetryAlert } from '@/components/hookbox/retry-alert'
import { ServedByChip, type ServedBy } from '@/components/hookbox/served-by-chip'
import { StatusCode } from '@/components/hookbox/status-code'
import { FullPageMessage } from '@/components/layout/full-page-message'
import { PublicHeader } from '@/components/layout/public-header'

const BUTTON_VARIANTS = ['primary', 'tint', 'ghost', 'outline', 'danger', 'danger-ghost', 'nav', 'link'] as const
const CARD_TONES: CardTone[] = ['surface', 'marigold', 'coral', 'sky', 'tint', 'peach', 'midnight']
const PILL_TONES: NonNullable<PillProps['tone']>[] = [
  'neutral', 'accent', 'success', 'warning', 'danger', 'info', 'marigold', 'coral', 'peach', 'sky',
]
const METHODS = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE', 'HEAD', 'OPTIONS']
const STATUSES = [200, 201, 204, 301, 304, 400, 404, 429, 500, 503]
const SERVED: ServedBy[] = ['rule', 'crud', 'mitm', 'tunnel', 'default', 'cors', 'chaos', 'ratelimit']
const CONN: ConnState[] = ['connecting', 'live', 'reconnecting', 'sse', 'offline', 'unauthorized', 'busy']

const SAMPLE: PublicRequestDetail = {
  id: 2,
  method: 'POST',
  path: '/orders',
  status_code: 201,
  served_by: 'crud',
  duration_ms: 8,
  timestamp: new Date(Date.now() - 60_000).toISOString(),
  request_headers: { 'content-type': 'application/json', authorization: '<redacted>' },
  query_params: { page: '2' },
  request_body: '{"sku":"A1","qty":2}',
  response_headers: { 'x-hookbox': 'crud' },
  response_body: '{"id":"42","sku":"A1"}',
}

interface HeaderRow {
  uid: string
  key: string
  value: string
}

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <Card as="section" className="space-y-3">
      <Overline as="h2">{title}</Overline>
      <div className="flex flex-wrap items-center gap-3">{children}</div>
    </Card>
  )
}

function FormSamples() {
  const [latency, setLatency] = useState(250)
  const [inheritable, setInheritable] = useState<number | null>(null)
  const [cors, setCors] = useState(true)
  const [rows, setRows] = useState<HeaderRow[]>([{ uid: 'h1', key: 'x-trace', value: 'abc' }])
  return (
    <Card as="section" className="grid gap-6 md:grid-cols-2">
      <div className="space-y-4">
        <Overline as="h2">Field system</Overline>
        <TextField label="Endpoint name" helper="Optional. Just a label for you." placeholder="e.g. Checkout API" />
        <TextField label="Target URL" mono error="Enter a full URL, including https://." defaultValue="api.example" />
        <SelectField
          label="Chaos mode"
          options={[
            { value: 'error', label: 'Return an error' },
            { value: 'dropout', label: 'Drop the connection' },
          ]}
        />
        <Field label="Response body" optional>
          <Textarea mono rows={3} defaultValue='{"ok": true}' />
        </Field>
        <SwitchField label="Auto-CORS" helper="Answer preflight requests for you." checked={cors} onCheckedChange={setCors} />
      </div>
      <div className="space-y-4">
        <Fieldset legend="Throttling" helper="Applies before the response is sent.">
          <NumberSlider label="Latency" unit="ms" min={0} max={10000} value={latency} onChange={setLatency} />
          <NumberSlider
            label="Rate limit"
            unit="per min"
            helper="Off inherits the endpoint setting."
            nullable
            min={0}
            max={6000}
            value={inheritable}
            onChange={setInheritable}
          />
        </Fieldset>
        <ListEditor
          legend="Response headers"
          rows={rows}
          onChange={setRows}
          createRow={() => ({ uid: crypto.randomUUID(), key: '', value: '' })}
          addLabel="Add header"
          columns={[
            {
              key: 'key',
              header: 'Header',
              render: (row, { label, update }) => (
                <Input mono aria-label={label} value={row.key} onChange={(e) => update({ key: e.target.value })} />
              ),
            },
            {
              key: 'value',
              header: 'Value',
              width: '2fr',
              render: (row, { label, update }) => (
                <Input mono aria-label={label} value={row.value} onChange={(e) => update({ value: e.target.value })} />
              ),
            },
          ]}
        />
      </div>
    </Card>
  )
}

function RequestSamples() {
  const [selected, setSelected] = useState(2)
  const [seg, setSeg] = useState<'pretty' | 'raw'>('pretty')
  const rows = [
    { ...SAMPLE, id: 1, method: 'GET', path: '/users/42', status_code: 200, served_by: 'rule' as ServedBy },
    SAMPLE,
    { ...SAMPLE, id: 3, method: 'DELETE', path: '/sessions/abc', status_code: 500, served_by: 'chaos' as ServedBy },
  ]
  return (
    <div className="grid gap-4 lg:grid-cols-2">
      <Card as="section" padding="none">
        <div className="p-4">
          <Overline as="h2">FeedRow (selected = rail + fill)</Overline>
        </div>
        <div role="listbox" aria-label="Sample feed">
          {rows.map((r) => (
            <FeedRow key={r.id} row={r} selected={selected === r.id} onSelect={setSelected} />
          ))}
        </div>
        <div className={`grid items-center gap-2 border-t border-border px-3 py-2.5 text-body-sm ${REQUEST_GRID_DISCLOSURE}`}>
          <span aria-hidden="true" />
          <RequestCells request={SAMPLE} wrapMeta />
        </div>
      </Card>
      <Card as="section" padding="none">
        <RequestSubject request={SAMPLE} className="border-b border-border px-4 py-2.5" />
        <RequestDetailTabs
          detail={SAMPLE}
          extraTabs={[{ value: 'trace', label: t('insp.tab.trace'), content: <p className="text-body-sm">matched rule 3</p> }]}
        />
      </Card>
      <Card as="section" className="space-y-2">
        <Overline as="h2">JsonTree + Segmented</Overline>
        <Segmented
          aria-label="View"
          options={[
            { value: 'pretty', label: 'Pretty' },
            { value: 'raw', label: 'Raw' },
          ]}
          value={seg}
          onChange={setSeg}
        />
        <JsonTree raw='{"id":"42","active":true,"roles":["admin","user"],"meta":{"v":2}}' />
      </Card>
      <Card as="section" className="space-y-2">
        <Overline as="h2">CodeBlock / MockUrlChip</Overline>
        <CodeBlock value="curl https://<your-host>/e/<token>/ping" />
        <MockUrlChip url="https://<your-host>/e/<token>" />
      </Card>
    </div>
  )
}

export function PrimitivesGallery() {
  return (
    <div className="min-h-screen bg-canvas">
      <PublicHeader actions={<Button variant="tint" size="sm">Tint action</Button>} />
      <main className="mx-auto max-w-page space-y-6 px-4 py-8 sm:px-6">
        <PageHeader
          eyebrow="Design system"
          title="HookBox primitives"
          description="Every primitive, variant and tone — check both themes."
          back={{ to: '/', label: 'Back to start' }}
          meta={<MockUrlChip url="https://ab12cd34.hookbox.test" />}
          actions={
            <Sheet>
              <SheetTrigger asChild>
                <Button variant="outline">Open sheet</Button>
              </SheetTrigger>
              <SheetContent title="Menu" description="A right-docked sheet.">
                <Button variant="nav" aria-current="page" className="w-full justify-start">
                  Requests
                </Button>
                <Button variant="nav" className="w-full justify-start">
                  Rules
                </Button>
              </SheetContent>
            </Sheet>
          }
        />

        <Section title="Buttons">
          {BUTTON_VARIANTS.map((variant) => (
            <Button key={variant} variant={variant} aria-current={variant === 'nav' ? 'page' : undefined}>
              {variant}
            </Button>
          ))}
          <Button size="sm">Small</Button>
          <Button size="lg">Large</Button>
          <Button loading>Saving…</Button>
          <Button disabled>Disabled</Button>
          <Spinner />
        </Section>

        <Section title="Cards">
          <div className="grid w-full gap-3 sm:grid-cols-2 lg:grid-cols-4">
            {CARD_TONES.map((tone) => (
              <Card key={tone} tone={tone}>
                <CardHeader title={tone} description="Accent feature card." titleAs="h3" />
              </Card>
            ))}
          </div>
        </Section>

        <Section title="Pills">
          {PILL_TONES.map((tone) => (
            <Pill key={tone} tone={tone}>
              {tone}
            </Pill>
          ))}
          <Pill tone="accent" size="md">
            md
          </Pill>
          <span className="text-h2">
            Watch <Pill tone="peach" size="hero">every</Pill> request
          </span>
        </Section>

        <Section title="MethodBadge · StatusCode · ServedByChip · ConnectionPill">
          {METHODS.map((m) => (
            <MethodBadge key={m} method={m} />
          ))}
          {STATUSES.map((s) => (
            <StatusCode key={s} code={s} />
          ))}
          {SERVED.map((s) => (
            <ServedByChip key={s} servedBy={s} />
          ))}
          {CONN.map((c) => (
            <ConnectionPill key={c} state={c} label={t(`feed.conn.${c}` as never)} />
          ))}
        </Section>

        <RequestSamples />
        <FormSamples />

        <SettingsSection title="Settings section" description="One titled card per settings group.">
          <p className="text-body-sm text-text-secondary">Section body.</p>
        </SettingsSection>
        <SettingsSection title="Danger zone" tone="danger">
          <Button variant="danger">Delete endpoint</Button>
        </SettingsSection>

        <div className="grid gap-4 md:grid-cols-2">
          <div className="space-y-3">
            <InlineAlert variant="info" title="Detail on its way">
              This request just landed — its detail is still being written.
            </InlineAlert>
            <InlineAlert variant="warning" title="Stored, not yet sent">
              HookBox saves this with the rule but doesn't fire it yet.
            </InlineAlert>
            <RetryAlert title="Couldn't load this request" onRetry={() => {}}>
              Something went wrong fetching the detail.
            </RetryAlert>
          </div>
          <Card padding="none">
            <EmptyState icon={<Inbox className="h-6 w-6" />} title="No requests yet" description="Send one to your mock URL." />
            <LoadingBlock label="Loading" lines={3} className="p-4" />
          </Card>
        </div>

        <Card padding="none">
          <FullPageMessage title="Page not found" description="That page doesn't exist." actions={<Button variant="link">Back to start</Button>} />
        </Card>
      </main>
    </div>
  )
}
