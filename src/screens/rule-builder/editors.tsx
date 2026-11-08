/**
 * The rule builder's repeated-row editors — thin ListEditor wrappers, each
 * a <fieldset> named by its legend with every cell labelled "{column} {n}".
 * Column names deliberately avoid the bare words "Path" and "Name" (e2e
 * substring label queries elsewhere in the dialog).
 */
import type { BodyCondition, StateRequirement } from '@/api/schemas'
import { t, type CopyKey } from '@/lib/copy'
import { Input } from '@/components/ui/input'
import { ListEditor, type ListEditorColumn } from '@/components/ui/list-editor'
import { Select, type SelectOption } from '@/components/ui/select'
import { newUid, type BodyConditionRow, type PairRow, type StateRequirementRow, type StateWriteRow } from './draft'

interface RowsProps<T> {
  rows: T[]
  onChange: (rows: T[]) => void
}

function textColumn<T extends { uid: string }>(
  key: keyof T & string,
  header: string,
  placeholder: string,
  isDisabled?: (row: T) => boolean,
): ListEditorColumn<T> {
  return {
    key,
    header,
    render: (row, cell) => (
      <Input
        mono
        aria-label={cell.label}
        placeholder={placeholder}
        disabled={isDisabled?.(row)}
        value={String(row[key] ?? '')}
        onChange={(e) => cell.update({ [key]: e.target.value } as Partial<T>)}
      />
    ),
  }
}

function opColumn<T extends { uid: string; op: string }>(options: readonly SelectOption[]): ListEditorColumn<T> {
  return {
    key: 'op',
    header: t('rule.col.operator'),
    width: '9rem',
    render: (row, cell) => (
      <Select
        aria-label={cell.label}
        options={options}
        value={row.op}
        onChange={(e) => cell.update({ op: e.target.value } as Partial<T>)}
      />
    ),
  }
}

const opOptions = (ops: readonly [string, CopyKey][]): SelectOption[] =>
  ops.map(([value, label]) => ({ value, label: t(label) }))

export function PairListEditor({
  legend,
  helper,
  keyHeader,
  valueHeader,
  keyPlaceholder,
  valuePlaceholder,
  addLabel,
  rows,
  onChange,
}: RowsProps<PairRow> & {
  legend: string
  helper?: string
  keyHeader: string
  valueHeader: string
  keyPlaceholder: string
  valuePlaceholder: string
  addLabel: string
}) {
  return (
    <ListEditor
      legend={legend}
      helper={helper}
      rows={rows}
      onChange={onChange}
      addLabel={addLabel}
      createRow={() => ({ uid: newUid(), key: '', value: '' })}
      columns={[
        textColumn<PairRow>('key', keyHeader, keyPlaceholder),
        textColumn<PairRow>('value', valueHeader, valuePlaceholder),
      ]}
    />
  )
}

const BODY_OPS = opOptions([
  ['eq', 'rule.match.body.op.eq'],
  ['neq', 'rule.match.body.op.neq'],
  ['contains', 'rule.match.body.op.contains'],
  ['exists', 'rule.match.body.op.exists'],
])

export function BodyConditionsEditor({ rows, onChange }: RowsProps<BodyConditionRow>) {
  return (
    <ListEditor
      legend={t('rule.match.body.label')}
      helper={t('rule.match.body.helper')}
      rows={rows}
      onChange={onChange}
      addLabel={t('rule.match.body.add')}
      createRow={() => ({ uid: newUid(), path: '', op: 'eq' as BodyCondition['op'], value: '' })}
      columns={[
        textColumn<BodyConditionRow>('path', t('rule.col.field'), t('rule.match.body.pathPlaceholder')),
        opColumn<BodyConditionRow>(BODY_OPS),
        textColumn<BodyConditionRow>('value', t('rule.col.value'), t('rule.match.body.valPlaceholder'), (row) => row.op === 'exists'),
      ]}
    />
  )
}

const STATE_OPS = opOptions([
  ['eq', 'rule.match.state.op.eq'],
  ['neq', 'rule.match.state.op.neq'],
  ['exists', 'rule.match.state.op.exists'],
  ['absent', 'rule.match.state.op.absent'],
])

export function StateRequirementsEditor({ rows, onChange }: RowsProps<StateRequirementRow>) {
  return (
    <ListEditor
      legend={t('rule.match.state.label')}
      helper={t('rule.match.state.helper')}
      rows={rows}
      onChange={onChange}
      addLabel={t('rule.match.state.add')}
      createRow={() => ({ uid: newUid(), key: '', op: 'eq' as StateRequirement['op'], value: '' })}
      columns={[
        textColumn<StateRequirementRow>('key', t('rule.col.stateKey'), t('rule.match.state.keyPlaceholder')),
        opColumn<StateRequirementRow>(STATE_OPS),
        textColumn<StateRequirementRow>(
          'value',
          t('rule.col.value'),
          t('rule.match.state.valPlaceholder'),
          (row) => row.op === 'exists' || row.op === 'absent',
        ),
      ]}
    />
  )
}

export function StateWritesEditor({ rows, onChange }: RowsProps<StateWriteRow>) {
  return (
    <ListEditor
      legend={t('rule.act.stateWrites.label')}
      helper={t('rule.act.stateWrites.helper')}
      rows={rows}
      onChange={onChange}
      addLabel={t('rule.act.stateWrites.add')}
      createRow={() => ({ uid: newUid(), key: '', value: '' })}
      columns={[
        textColumn<StateWriteRow>('key', t('rule.col.stateKey'), t('rule.act.stateWrites.keyPlaceholder')),
        textColumn<StateWriteRow>('value', t('rule.col.value'), t('rule.act.stateWrites.valPlaceholder')),
      ]}
    />
  )
}
