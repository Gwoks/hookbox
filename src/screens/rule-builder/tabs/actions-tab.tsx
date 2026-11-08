/** Actions → state_writes[] + webhook_action. The webhook is stored with
 * the rule but not fired yet, so it carries a "Stored, not yet sent"
 * warning pill (AC-J10) — the shape still round-trips. */
import { t } from '@/lib/copy'
import { Field, Fieldset } from '@/components/ui/field'
import { Textarea, TextField } from '@/components/ui/input'
import { Pill } from '@/components/ui/pill'
import { StateWritesEditor } from '../editors'
import type { TabProps } from './types'

export function ActionsTab({ draft, set }: TabProps) {
  return (
    <div className="space-y-8">
      <StateWritesEditor rows={draft.state_writes} onChange={(rows) => set('state_writes', rows)} />
      <Fieldset
        legend={
          <span className="flex flex-wrap items-center gap-2">
            {t('rule.act.webhook.label')}
            <Pill tone="warning">{t('rule.act.webhook.badge')}</Pill>
          </span>
        }
        helper={t('rule.act.webhook.helper')}
      >
        <TextField
          label={t('rule.act.webhook.url.label')}
          mono
          type="url"
          placeholder={t('rule.act.webhook.urlPlaceholder')}
          value={draft.webhook_url}
          onChange={(e) => set('webhook_url', e.target.value)}
        />
        <Field label={t('rule.act.webhook.body.label')}>
          <Textarea
            mono
            rows={3}
            placeholder={t('rule.act.webhook.bodyPlaceholder')}
            value={draft.webhook_body}
            onChange={(e) => set('webhook_body', e.target.value)}
          />
        </Field>
      </Fieldset>
    </div>
  )
}
