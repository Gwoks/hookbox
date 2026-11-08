/** Auto-CORS: answer OPTIONS preflights and reflect the Origin on the mock
 * surface. */
import { t } from '@/lib/copy'
import { SettingsSection } from '@/components/ui/section'
import { SwitchField } from '@/components/ui/switch'
import type { SettingsFormState } from '../use-settings-form'

export function CorsSection({ form }: { form: SettingsFormState }) {
  return (
    <SettingsSection id="cors" title={t('set.cors.title')}>
      <SwitchField
        label={t('set.cors.toggle.label')}
        helper={t('set.cors.helper')}
        checked={form.draft.cors_enabled}
        onCheckedChange={(on) => form.set('cors_enabled', on)}
      />
    </SettingsSection>
  )
}
