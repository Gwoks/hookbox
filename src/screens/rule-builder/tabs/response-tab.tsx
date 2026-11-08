/** Response → ResponseSpec { status_code, content_type, headers,
 * body_template }. The body counter shows UTF-8 bytes against the 256 KB
 * cap: warning colour near it, danger over it, always with text (AC-D24).
 * The textarea reports its caret so Templating can insert at it. */
import { useId } from 'react'
import { cn } from '@/lib/cn'
import { t } from '@/lib/copy'
import { Label } from '@/components/ui/field'
import { Textarea, TextField } from '@/components/ui/input'
import { BODY_CAP_BYTES, BODY_WARN_BYTES, utf8Bytes, type TextRange } from '../draft'
import { PairListEditor } from '../editors'
import type { TabProps } from './types'

const formatKb = (bytes: number) => (bytes / 1024).toFixed(1)

function counterClass(bytes: number): string {
  if (bytes > BODY_CAP_BYTES) return 'text-danger-fg'
  if (bytes >= BODY_WARN_BYTES) return 'text-warning-fg'
  return 'text-text-tertiary'
}

export function ResponseTab({ draft, set, errors, onCaret }: TabProps & { onCaret: (range: TextRange) => void }) {
  const bodyId = useId()
  const bytes = utf8Bytes(draft.body_template)
  const counter =
    bytes > BODY_CAP_BYTES
      ? t('rule.resp.body.counter.over', { used: formatKb(bytes) })
      : t('rule.resp.body.counter', { used: formatKb(bytes) })
  const trackCaret = (el: HTMLTextAreaElement) => onCaret({ start: el.selectionStart, end: el.selectionEnd })

  return (
    <div className="space-y-6">
      <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
        <TextField
          label={t('rule.resp.status.label')}
          helper={t('rule.resp.status.helper')}
          error={errors.status_code}
          type="number"
          inputMode="numeric"
          value={Number.isNaN(draft.status_code) ? '' : draft.status_code}
          onChange={(e) => set('status_code', e.target.valueAsNumber)}
        />
        <TextField
          label={t('rule.resp.contentType.label')}
          mono
          value={draft.content_type}
          onChange={(e) => set('content_type', e.target.value)}
        />
      </div>
      <PairListEditor
        legend={t('rule.resp.headers.label')}
        keyHeader={t('rule.col.header')}
        valueHeader={t('rule.col.value')}
        keyPlaceholder={t('rule.match.headers.keyPlaceholder')}
        valuePlaceholder={t('rule.resp.headers.valPlaceholder')}
        addLabel={t('rule.resp.headers.add')}
        rows={draft.resp_headers}
        onChange={(rows) => set('resp_headers', rows)}
      />
      <div className="space-y-1.5">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <Label htmlFor={bodyId}>{t('rule.resp.body.label')}</Label>
          <span className={cn('tnum text-caption', counterClass(bytes))} aria-live="polite">
            {counter}
          </span>
        </div>
        <Textarea
          id={bodyId}
          mono
          rows={8}
          invalid={Boolean(errors.body_template)}
          aria-describedby={errors.body_template ? `${bodyId}-error` : undefined}
          placeholder={t('rule.resp.body.placeholder')}
          value={draft.body_template}
          onChange={(e) => {
            set('body_template', e.target.value)
            trackCaret(e.currentTarget)
          }}
          onSelect={(e) => trackCaret(e.currentTarget)}
        />
        {errors.body_template && (
          <p id={`${bodyId}-error`} className="text-body-sm text-danger-fg" role="alert">
            {errors.body_template}
          </p>
        )}
      </div>
    </div>
  )
}
