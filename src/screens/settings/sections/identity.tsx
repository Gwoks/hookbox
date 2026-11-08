/** Identity: the editable endpoint name, then the copy-only Mock URL, local
 * /e/ path and token (AC-D19: rendered as <code>, never links). */
import type { EndpointDetail } from '@/api'
import { t } from '@/lib/copy'
import { absolutize } from '@/lib/url'
import { CodeBlock } from '@/components/hookbox/code-block'
import { TextField } from '@/components/ui/input'
import { SettingsSection } from '@/components/ui/section'
import type { SettingsFormState } from '../use-settings-form'

function ReadOnlyValue({ label, value }: { label: string; value: string }) {
  return (
    <div className="space-y-1.5">
      <p className="text-body-sm font-medium text-text-primary">{label}</p>
      <CodeBlock value={value} ariaLabel={label} />
    </div>
  )
}

export function IdentitySection({ form, endpoint }: { form: SettingsFormState; endpoint: EndpointDetail }) {
  return (
    <SettingsSection id="identity" title={t('set.identity.title')}>
      <TextField
        label={t('set.identity.name.label')}
        helper={t('set.identity.name.helper')}
        placeholder={t('set.identity.name.placeholder')}
        value={form.draft.name}
        onChange={(e) => form.set('name', e.target.value)}
      />
      <ReadOnlyValue label={t('set.identity.mockUrl.label')} value={absolutize(endpoint.mock_url)} />
      <ReadOnlyValue label={t('set.identity.pathUrl.label')} value={absolutize(endpoint.path_url)} />
      <ReadOnlyValue label={t('set.identity.token.label')} value={endpoint.token} />
    </SettingsSection>
  )
}
