/** The owner-only "State & Tracing" tab: the resolution trace steps and the
 * endpoint state snapshot at request time. Every value renders as text. */
import type { RequestDetail } from '@/api'
import { t } from '@/lib/copy'
import { KeyValueRows } from '@/components/hookbox/key-value-rows'
import { Overline } from '@/components/ui/overline'

export function TracePanel({ detail }: { detail: RequestDetail }) {
  return (
    <>
      <section className="space-y-1.5">
        <Overline as="h3">{t('insp.trace.title')}</Overline>
        {detail.trace.length > 0 ? (
          <ol className="space-y-1">
            {detail.trace.map((step, i) => (
              <li key={i} className="flex gap-2 rounded-sm bg-surface-subtle px-2 py-1.5 font-mono text-mono-sm">
                <span className="shrink-0 font-medium text-text-secondary">{step.step}</span>
                <span className="break-all text-text-primary">{step.detail}</span>
              </li>
            ))}
          </ol>
        ) : (
          <p className="text-body-sm text-text-tertiary">{t('insp.trace.empty')}</p>
        )}
      </section>
      <section className="space-y-1.5">
        <Overline as="h3">{t('insp.trace.stateTitle')}</Overline>
        <KeyValueRows data={detail.state_snapshot} emptyLabel={t('insp.trace.stateEmpty')} />
      </section>
    </>
  )
}
