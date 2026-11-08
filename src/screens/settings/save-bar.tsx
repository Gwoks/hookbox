/**
 * SaveBar — the settings form's sticky footer (a floating surface, so it
 * may carry a shadow): the unsaved-changes indicator, a save failure,
 * Discard, and the screen's one primary. Save is ALWAYS enabled: Save on a
 * clean form re-sends the current values (AC-89b clicks it right after an
 * import).
 */
import { t } from '@/lib/copy'
import { Button } from '@/components/ui/button'
import type { SettingsFormState } from './use-settings-form'

export function SaveBar({ form }: { form: SettingsFormState }) {
  return (
    <div className="sticky bottom-4 z-sticky flex flex-wrap items-center gap-3 rounded-md border border-border bg-surface px-4 py-3 shadow-md">
      <div className="min-w-0 flex-1 text-body-sm" aria-live="polite">
        {form.saveError ? (
          <p role="alert" className="text-danger-fg">
            {form.saveError}
          </p>
        ) : form.isDirty ? (
          <p className="flex items-center gap-2 font-medium text-text-primary">
            <span className="h-2 w-2 rounded-pill bg-warning-fg" aria-hidden="true" />
            {t('set.saveBar.dirty')}
          </p>
        ) : (
          <p className="text-text-tertiary">{t('set.saveBar.clean')}</p>
        )}
      </div>
      <Button variant="ghost" onClick={form.discard} disabled={!form.isDirty || form.isSaving}>
        {t('set.discard')}
      </Button>
      <Button variant="primary" onClick={() => void form.submit()} loading={form.isSaving}>
        {form.isSaving ? t('set.saving') : t('set.save')}
      </Button>
    </div>
  )
}
