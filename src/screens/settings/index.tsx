/**
 * Endpoint settings (/d/:token/settings) — PRD §5.2 #5/#6/#14/#16/#17/#18,
 * AC-49..52, AC-J9, copy.md §5.9. One SettingsSection card per group in a
 * doc-width column; at lg a sticky section index sits beside it. The
 * endpoint form (identity → CORS) saves through the sticky SaveBar — the
 * screen's one primary; the other sections act on their own, each
 * destructive one behind a confirm. The owner-level Recovery key card keeps
 * its `#recovery-key` anchor (account menu, the dashboard's save-key nudge).
 */
import { t, type CopyKey } from '@/lib/copy'
import { useEndpoints } from '@/components/owner/endpoint-context'
import { RecoveryKeySection, RECOVERY_KEY_SECTION_ID } from '@/components/owner/recovery-key-section'
import { Button } from '@/components/ui/button'
import { PageHeader } from '@/components/ui/page-header'
import { SaveBar } from './save-bar'
import { AutoCrudSection } from './sections/auto-crud'
import { ConditionsSection } from './sections/conditions'
import { ConfigurationSection } from './sections/configuration'
import { CorsSection } from './sections/cors'
import { DangerZoneSection } from './sections/danger-zone'
import { DefaultResponseSection } from './sections/default-response'
import { IdentitySection } from './sections/identity'
import { ProxySection } from './sections/proxy'
import { RetentionSection } from './sections/retention'
import { useSettingsForm } from './use-settings-form'

const SECTION_INDEX: readonly { id: string; label: CopyKey }[] = [
  { id: 'identity', label: 'set.identity.title' },
  { id: 'proxy', label: 'set.proxy.title' },
  { id: 'auto-crud', label: 'set.crud.title' },
  { id: 'default-response', label: 'set.default.title' },
  { id: 'conditions', label: 'set.cond.title' },
  { id: 'cors', label: 'set.cors.title' },
  { id: 'configuration', label: 'set.config.title' },
  { id: 'retention', label: 'set.retention.title' },
  { id: RECOVERY_KEY_SECTION_ID, label: 'set.recovery.title' },
  { id: 'danger-zone', label: 'set.danger.title' },
]

function SectionIndex() {
  return (
    <nav aria-label={t('set.nav.aria')} className="sticky top-[calc(theme(spacing.nav)+2rem)] hidden lg:block">
      <ul className="space-y-0.5">
        {SECTION_INDEX.map((section) => (
          <li key={section.id}>
            <Button variant="nav" size="sm" asChild className="w-full justify-start">
              <a href={`#${section.id}`}>{t(section.label)}</a>
            </Button>
          </li>
        ))}
      </ul>
    </nav>
  )
}

export function Settings() {
  const { token, endpoint, replace } = useEndpoints()
  const form = useSettingsForm()

  return (
    <div className="mx-auto w-full max-w-wide space-y-8 px-4 py-8 sm:px-6">
      <PageHeader eyebrow={endpoint.name || endpoint.token} title={t('set.title')} />
      <div className="lg:grid lg:grid-cols-[12rem_minmax(0,1fr)] lg:items-start lg:gap-10">
        <SectionIndex />
        <div className="max-w-doc space-y-6">
          <IdentitySection form={form} endpoint={endpoint} />
          <ProxySection form={form} />
          <AutoCrudSection form={form} token={token} />
          <DefaultResponseSection form={form} />
          <ConditionsSection form={form} />
          <CorsSection form={form} />
          <SaveBar form={form} />
          <ConfigurationSection token={token} endpoint={endpoint} isFormDirty={form.isDirty} onImported={replace} />
          <RetentionSection token={token} />
          <RecoveryKeySection />
          <DangerZoneSection token={token} />
        </div>
      </div>
    </div>
  )
}
