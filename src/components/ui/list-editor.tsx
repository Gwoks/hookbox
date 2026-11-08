/**
 * ListEditor<T> — the shared editor for repeated rows (headers, query
 * params, body conditions, state requirements/writes). A <fieldset> named by
 * its legend; visible column headers are decorative (aria-hidden) because
 * every cell is labelled "{column} {n}" — callers must avoid the bare words
 * "Path"/"Name" in column headers, which collide with Playwright substring
 * label queries elsewhere on the page. All edits go through ONE immutable
 * `update(patch)` per row; rows are keyed by a stable `uid`.
 */
import { Plus, X } from 'lucide-react'
import { useId, type ReactNode } from 'react'
import { cn } from '@/lib/cn'
import { t } from '@/lib/copy'
import { Button } from './button'
import { FieldContext } from './field'

export interface ListEditorCell<T> {
  /** Accessible name for the cell's control: "{column} {n}". */
  label: string
  index: number
  update: (patch: Partial<T>) => void
}

export interface ListEditorColumn<T> {
  key: string
  header: string
  /** CSS grid track; defaults to minmax(0,1fr). */
  width?: string
  render: (row: T, cell: ListEditorCell<T>) => ReactNode
}

export interface ListEditorProps<T extends { uid: string }> {
  legend: ReactNode
  hideLegend?: boolean
  helper?: ReactNode
  rows: readonly T[]
  columns: readonly ListEditorColumn<T>[]
  onChange: (rows: T[]) => void
  createRow: () => T
  addLabel: string
  removeLabel?: (n: number) => string
  emptyLabel?: ReactNode
  className?: string
}

const REMOVE_TRACK = '2rem'

export function ListEditor<T extends { uid: string }>({
  legend,
  hideLegend,
  helper,
  rows,
  columns,
  onChange,
  createRow,
  addLabel,
  removeLabel = (n) => t('common.list.remove', { n }),
  emptyLabel = t('common.list.empty'),
  className,
}: ListEditorProps<T>) {
  const helperId = useId()
  const gridTemplateColumns = [...columns.map((c) => c.width ?? 'minmax(0,1fr)'), REMOVE_TRACK].join(' ')

  const updateAt = (index: number) => (patch: Partial<T>) =>
    onChange(rows.map((row, i) => (i === index ? { ...row, ...patch } : row)))
  const removeAt = (index: number) => onChange(rows.filter((_, i) => i !== index))

  return (
    <fieldset
      className={cn('min-w-0 space-y-2', className)}
      aria-describedby={helper ? helperId : undefined}
    >
      <legend className={cn('text-body-sm font-medium text-text-primary', hideLegend && 'sr-only')}>{legend}</legend>
      {helper && (
        <p id={helperId} className="text-body-sm text-text-tertiary">
          {helper}
        </p>
      )}
      <FieldContext.Provider value={null}>
        {rows.length === 0 ? (
          <p className="text-body-sm text-text-tertiary">{emptyLabel}</p>
        ) : (
          <div className="space-y-2">
            <div className="grid gap-2" style={{ gridTemplateColumns }} aria-hidden="true">
              {columns.map((col) => (
                <span key={col.key} className="text-caption text-text-tertiary">
                  {col.header}
                </span>
              ))}
            </div>
            {rows.map((row, index) => (
              <div key={row.uid} className="grid items-center gap-2" style={{ gridTemplateColumns }}>
                {columns.map((col) => (
                  <div key={col.key} className="min-w-0">
                    {col.render(row, { label: `${col.header} ${index + 1}`, index, update: updateAt(index) })}
                  </div>
                ))}
                <Button
                  type="button"
                  variant="ghost"
                  size="icon-sm"
                  aria-label={removeLabel(index + 1)}
                  onClick={() => removeAt(index)}
                >
                  <X className="h-4 w-4" aria-hidden="true" />
                </Button>
              </div>
            ))}
          </div>
        )}
      </FieldContext.Provider>
      <Button type="button" variant="outline" size="sm" onClick={() => onChange([...rows, createRow()])}>
        <Plus className="h-3.5 w-3.5" aria-hidden="true" />
        {addLabel}
      </Button>
    </fieldset>
  )
}
