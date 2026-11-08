/** AC-S21's pre-apply summary: provenance → only the settings that change
 * (target_url highlighted, with its consequence) → how many rules get
 * added → the request-header-tag and unsaved-form warnings. */
import { t } from '@/lib/copy'
import { cn } from '@/lib/cn'
import { countRulesUsingRequestHeaderTag, type ConfigBundle, type ConfigDiffRow } from '@/lib/config-bundle'

export interface ImportConfirmBodyProps {
  bundle: ConfigBundle
  diff: ConfigDiffRow[]
  existingRuleCount: number
  isFormDirty: boolean
}

function DiffRow({ row }: { row: ConfigDiffRow }) {
  const from = row.from ?? t('set.config.diff.empty')
  const to = row.to ?? t('set.config.diff.empty')
  return (
    <div
      className={cn(
        'grid grid-cols-[8rem_minmax(0,1fr)] gap-2 px-2.5 py-1.5',
        row.field === 'target_url' && 'bg-warning-bg',
      )}
    >
      <span className="font-mono text-mono-sm text-text-secondary">{row.field}</span>
      <span
        className="min-w-0 break-all font-mono text-mono-sm"
        aria-label={t('set.config.diff.change.aria', { field: row.field, from, to })}
      >
        <span className="text-text-tertiary line-through">{from}</span>
        <span className="px-1 text-text-tertiary" aria-hidden="true">
          →
        </span>
        <span className="text-text-primary">{to}</span>
      </span>
    </div>
  )
}

export function ImportConfirmBody({ bundle, diff, existingRuleCount, isFormDirty }: ImportConfirmBodyProps) {
  const when = new Date(bundle.exported_at).toLocaleString()
  const headerTagCount = countRulesUsingRequestHeaderTag(bundle.rules)
  const isTargetChanging = diff.some((row) => row.field === 'target_url')

  return (
    <div className="space-y-3">
      <p className="text-body-sm text-text-secondary">
        {bundle.endpoint.name
          ? t('set.config.confirm.exported', { when, name: bundle.endpoint.name })
          : t('set.config.confirm.exported.unnamed', { when })}
      </p>
      {diff.length === 0 ? (
        <p className="text-body-sm text-text-tertiary">{t('set.config.diff.none')}</p>
      ) : (
        <div className="space-y-1.5">
          <p className="text-body-sm font-medium text-text-primary">{t('set.config.diff.title', { n: diff.length })}</p>
          <div className="divide-y divide-border overflow-hidden rounded-sm border border-border">
            {diff.map((row) => (
              <DiffRow key={row.field} row={row} />
            ))}
          </div>
          {isTargetChanging && <p className="text-caption text-warning-fg">{t('set.config.diff.targetUrl.warning')}</p>}
          <p className="text-caption text-text-tertiary">{t('set.config.diff.unchangedNote')}</p>
        </div>
      )}
      <p className="text-body-sm text-text-secondary">
        {bundle.rules.length > 0
          ? t('set.config.confirm.rules', { n: bundle.rules.length, existing: existingRuleCount })
          : t('set.config.confirm.rules.none')}
      </p>
      {headerTagCount > 0 && (
        <p className="text-caption text-warning-fg">{t('set.config.confirm.headerTagWarning', { n: headerTagCount })}</p>
      )}
      {isFormDirty && <p className="text-body-sm text-warning-fg">{t('set.config.confirm.dirty')}</p>}
    </div>
  )
}
