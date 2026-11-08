/** Default response: what the endpoint returns when nothing else matches —
 * a 404 no-match body, or an echo of the request. */
import { t } from '@/lib/copy'
import { SettingsSection } from '@/components/ui/section'
import { Segmented } from '@/components/ui/segmented'
import type { SettingsFormState } from '../use-settings-form'

export function DefaultResponseSection({ form }: { form: SettingsFormState }) {
  const isEcho = form.draft.default_mode === 'echo'
  return (
    <SettingsSection id="default-response" title={t('set.default.title')} description={t('set.default.helper')}>
      <Segmented
        aria-label={t('set.default.title')}
        options={[
          { value: 'mock_404', label: t('set.default.mock404.label') },
          { value: 'echo', label: t('set.default.echo.label') },
        ]}
        value={form.draft.default_mode}
        onChange={(mode) => form.set('default_mode', mode)}
      />
      <p className="text-caption text-text-tertiary">
        {isEcho ? t('set.default.echo.helper') : t('set.default.mock404.helper')}
      </p>
    </SettingsSection>
  )
}
