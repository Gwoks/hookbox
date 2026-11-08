/** Proxy target (MITM): where unmatched requests are forwarded. Validated
 * on blur and on Save. */
import { t } from '@/lib/copy'
import { TextField } from '@/components/ui/input'
import { SettingsSection } from '@/components/ui/section'
import type { SettingsFormState } from '../use-settings-form'

export function ProxySection({ form }: { form: SettingsFormState }) {
  return (
    <SettingsSection id="proxy" title={t('set.proxy.title')}>
      <TextField
        label={t('set.proxy.url.label')}
        helper={t('set.proxy.url.helper')}
        error={form.errors.target_url}
        mono
        type="url"
        placeholder={t('set.proxy.url.placeholder')}
        value={form.draft.target_url}
        onChange={(e) => form.set('target_url', e.target.value)}
        onBlur={form.revealErrors}
      />
      <p className="text-caption text-text-tertiary">{t('set.proxy.url.note')}</p>
    </SettingsSection>
  )
}
