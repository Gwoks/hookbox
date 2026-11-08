/**
 * Rules (/d/:token/rules) — an endpoint's rules, and the RuleBuilder for
 * create / edit (PRD §5.2 #7–#11, AC-14, copy.md §5.8). States: loading ·
 * error + Retry · empty (the fall-through honesty note) · list. The page
 * header owns the one primary ("New rule"); "Add default rule" is outline.
 * Opening with ?new=1 (the nav's "New rule") opens the builder, then
 * strips the param.
 */
import { Plus } from 'lucide-react'
import { useEffect, useState } from 'react'
import { useSearchParams } from 'react-router-dom'
import type { MockRule } from '@/api'
import { t } from '@/lib/copy'
import { ConfirmDialog } from '@/components/hookbox/confirm-dialog'
import { RetryAlert } from '@/components/hookbox/retry-alert'
import { useEndpoints } from '@/components/owner/endpoint-context'
import { Button } from '@/components/ui/button'
import { Card } from '@/components/ui/card'
import { EmptyState } from '@/components/ui/empty-state'
import { LoadingBlock } from '@/components/ui/loading-block'
import { PageHeader } from '@/components/ui/page-header'
import { RuleBuilder } from '@/screens/rule-builder'
import { AddDefaultRuleButton } from './add-default-rule-button'
import { activeShadowedFallbacks, findCatchAllRule } from './default-rule'
import { RulesTable } from './rules-table'
import { useRules } from './use-rules'

type Builder = { open: false } | { open: true; rule: MockRule | null }

export function RulesManager() {
  const { token, endpoint } = useEndpoints()
  const rules = useRules(token)
  const [search, setSearch] = useSearchParams()
  const [builder, setBuilder] = useState<Builder>({ open: false })
  const [deleting, setDeleting] = useState<MockRule | null>(null)
  const [shadowConfirmOpen, setShadowConfirmOpen] = useState(false)

  useEffect(() => {
    if (search.get('new') !== '1') return
    setBuilder({ open: true, rule: null })
    const next = new URLSearchParams(search)
    next.delete('new')
    setSearch(next, { replace: true })
  }, [search, setSearch])

  const { list } = rules
  const ruleList = list.status === 'ready' ? (list.data ?? []) : null
  const catchAll = ruleList ? findCatchAllRule(ruleList) : undefined
  const shadowBullets = activeShadowedFallbacks(endpoint)
  const addDefault = {
    isDisabled: !ruleList || Boolean(catchAll) || rules.isAddingDefault,
    reason: catchAll ? (catchAll.enabled ? t('rules.default.exists') : t('rules.default.existsDisabled')) : null,
    isAdding: rules.isAddingDefault,
    onClick: () => (shadowBullets.length > 0 ? setShadowConfirmOpen(true) : void rules.addDefaultDirect()),
  }

  return (
    <div className="mx-auto w-full max-w-content space-y-6 px-4 py-8 sm:px-6">
      <PageHeader
        eyebrow={endpoint.name || endpoint.token}
        title={t('rules.title')}
        description={t('rules.description')}
        actions={
          <>
            <AddDefaultRuleButton {...addDefault} />
            <Button variant="primary" onClick={() => setBuilder({ open: true, rule: null })}>
              <Plus className="h-4 w-4" aria-hidden="true" />
              {t('rules.newRule')}
            </Button>
          </>
        }
      />

      {list.status === 'loading' && (
        <Card>
          <LoadingBlock label={t('rules.loading.aria')} lines={6} />
        </Card>
      )}
      {list.status === 'error' && <RetryAlert title={t('rules.error.title')} onRetry={() => void list.reload()} />}
      {ruleList && ruleList.length === 0 && (
        <Card padding="none">
          <EmptyState
            title={t('rules.empty.title')}
            description={t('rules.empty.body')}
            action={
              <div className="flex flex-col items-center gap-2">
                <AddDefaultRuleButton {...addDefault} />
                <p className="max-w-md text-caption text-text-tertiary">{t('rules.default.helper')}</p>
              </div>
            }
          />
        </Card>
      )}
      {ruleList && ruleList.length > 0 && (
        <RulesTable
          rules={ruleList}
          onEdit={(rule) => setBuilder({ open: true, rule })}
          onDuplicate={(rule) => void rules.duplicate(rule)}
          onDelete={setDeleting}
          onToggle={(rule, enabled) => void rules.toggle(rule, enabled)}
        />
      )}

      {builder.open && (
        <RuleBuilder
          token={token}
          rule={builder.rule}
          open
          onOpenChange={(open) => !open && setBuilder({ open: false })}
          onSaved={() => void rules.refresh()}
        />
      )}

      <ConfirmDialog
        open={deleting !== null}
        onClose={() => setDeleting(null)}
        title={t('rules.delete.title')}
        body={t('rules.delete.body', { name: deleting?.name || t('rules.row.unnamed') })}
        confirmLabel={t('rules.delete.confirm')}
        cancelLabel={t('rules.delete.cancel')}
        errorFallback={t('rule.error.save')}
        onConfirm={async () => {
          if (deleting) await rules.remove(deleting)
        }}
      />

      {/* F6 shadow confirm — only when >= 1 fallback is actually active
       * (AC-122). Primary, not danger: this may be exactly what the
       * operator wants, and it is recoverable. */}
      <ConfirmDialog
        open={shadowConfirmOpen}
        onClose={() => setShadowConfirmOpen(false)}
        title={t('rules.default.shadow.title')}
        body={
          <>
            <p>{t('rules.default.shadow.body')}</p>
            <ul className="mt-2 list-disc space-y-1 pl-5">
              {shadowBullets.map((bullet) => (
                <li key={bullet}>{bullet}</li>
              ))}
            </ul>
            <p className="mt-2 text-caption text-text-tertiary">{t('rules.default.shadow.recover')}</p>
          </>
        }
        confirmLabel={t('rules.default.shadow.confirm')}
        confirmVariant="primary"
        errorFallback={t('rules.default.error')}
        onConfirm={rules.addDefaultConfirmed}
      />
    </div>
  )
}
