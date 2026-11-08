/** Matching → MatchCriteria { method, path, headers, query, body_conditions,
 * state_requirements }. */
import { t } from '@/lib/copy'
import { TextField } from '@/components/ui/input'
import { SelectField } from '@/components/ui/select'
import { METHODS } from '../draft'
import { BodyConditionsEditor, PairListEditor, StateRequirementsEditor } from '../editors'
import type { TabProps } from './types'

const METHOD_OPTIONS = METHODS.map((m) => ({ value: m, label: m }))

export function MatchingTab({ draft, set, errors }: TabProps) {
  return (
    <div className="space-y-6">
      <div className="grid grid-cols-1 gap-3 sm:grid-cols-[9rem_minmax(0,1fr)]">
        <SelectField
          label={t('rule.match.method.label')}
          options={METHOD_OPTIONS}
          value={draft.method}
          onChange={(e) => set('method', e.target.value)}
        />
        <TextField
          label={t('rule.match.path.label')}
          helper={t('rule.match.path.helper')}
          error={errors.path}
          mono
          placeholder={t('rule.match.path.placeholder')}
          value={draft.path}
          onChange={(e) => set('path', e.target.value)}
        />
      </div>
      <PairListEditor
        legend={t('rule.match.headers.label')}
        helper={t('rule.match.headers.helper')}
        keyHeader={t('rule.col.header')}
        valueHeader={t('rule.col.expected')}
        keyPlaceholder={t('rule.match.headers.keyPlaceholder')}
        valuePlaceholder={t('rule.match.headers.valPlaceholder')}
        addLabel={t('rule.match.headers.add')}
        rows={draft.headers}
        onChange={(rows) => set('headers', rows)}
      />
      <PairListEditor
        legend={t('rule.match.query.label')}
        keyHeader={t('rule.col.param')}
        valueHeader={t('rule.col.expected')}
        keyPlaceholder={t('rule.match.query.keyPlaceholder')}
        valuePlaceholder={t('rule.match.query.valPlaceholder')}
        addLabel={t('rule.match.query.add')}
        rows={draft.query}
        onChange={(rows) => set('query', rows)}
      />
      <BodyConditionsEditor rows={draft.body_conditions} onChange={(rows) => set('body_conditions', rows)} />
      <StateRequirementsEditor rows={draft.state_requirements} onChange={(rows) => set('state_requirements', rows)} />
    </div>
  )
}
