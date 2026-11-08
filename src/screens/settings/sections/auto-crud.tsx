/** Auto-CRUD: the REST store toggle, plus the collection peek / clear. */
import { t } from '@/lib/copy'
import { SettingsSection } from '@/components/ui/section'
import { SwitchField } from '@/components/ui/switch'
import type { SettingsFormState } from '../use-settings-form'
import { CollectionPeek } from './collection-peek'

export function AutoCrudSection({ form, token }: { form: SettingsFormState; token: string }) {
  return (
    <SettingsSection id="auto-crud" title={t('set.crud.title')}>
      <SwitchField
        label={t('set.crud.toggle.label')}
        checked={form.draft.auto_crud}
        onCheckedChange={(on) => form.set('auto_crud', on)}
      />
      <div className="border-t border-border pt-4">
        <CollectionPeek token={token} />
      </div>
    </SettingsSection>
  )
}
