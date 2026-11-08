/**
 * RuleBuilder — the rule authoring dialog (PRD §5.3, AC-14, AC-D23/D24,
 * AC-J10, copy.md §5.7): an identity row (name, priority, enabled), then
 * five tabs mapping 1:1 to the frozen §5.3 shapes — Matching, Response,
 * Templating, Actions, Throttling. The working copy is a pure Draft
 * (./draft.ts); each tab trigger shows how many of its fields need
 * attention. Save validates first and jumps to the first tab with an
 * error; otherwise it sends `draftToPayload(draft)` to #8 create / #10
 * patch.
 */
import { useMemo, useRef, useState } from 'react'
import { api, ApiError, type MockRule, type MockRuleCreate } from '@/api'
import { t, type CopyKey } from '@/lib/copy'
import { useAsyncAction } from '@/lib/hooks/use-async-action'
import { useDraft } from '@/lib/hooks/use-draft'
import { InlineAlert } from '@/components/hookbox/inline-alert'
import { Button } from '@/components/ui/button'
import { Dialog, DialogBody, DialogContent, DialogFooter, DialogHeader } from '@/components/ui/dialog'
import { TextField } from '@/components/ui/input'
import { Pill } from '@/components/ui/pill'
import { SwitchField } from '@/components/ui/switch'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { useToast } from '@/components/ui/toast'
import {
  RULE_TABS,
  draftFromRule,
  draftToPayload,
  errorCountByTab,
  firstInvalidTab,
  insertAt,
  validateDraft,
  type RuleTab,
  type TextRange,
} from './draft'
import { ActionsTab } from './tabs/actions-tab'
import { MatchingTab } from './tabs/matching-tab'
import { ResponseTab } from './tabs/response-tab'
import { TemplatingTab } from './tabs/templating-tab'
import { ThrottlingTab } from './tabs/throttling-tab'

const TAB_LABELS: Record<RuleTab, CopyKey> = {
  matching: 'rule.tab.matching',
  response: 'rule.tab.response',
  templating: 'rule.tab.templating',
  actions: 'rule.tab.actions',
  throttling: 'rule.tab.throttling',
}

export interface RuleBuilderProps {
  token: string
  /** null = creating a new rule; a MockRule = editing it. */
  rule: MockRule | null
  open: boolean
  onOpenChange: (open: boolean) => void
  onSaved: () => void
}

function saveErrorMessage(error: unknown, isEditing: boolean): string {
  if (isEditing && error instanceof ApiError && error.status === 404) return t('rule.error.gone')
  return t('rule.error.save')
}

export function RuleBuilder({ token, rule, open, onOpenChange, onSaved }: RuleBuilderProps) {
  const { toast } = useToast()
  const initial = useMemo(() => draftFromRule(rule), [rule])
  const { draft, set } = useDraft(initial, { resetKey: rule?.id ?? 'new' })
  const [tab, setTab] = useState<RuleTab>('matching')
  // The body textarea's last selection, so a Templating tag lands where the
  // author left the caret even though that tab is not mounted.
  const caret = useRef<TextRange | null>(null)
  const save = useAsyncAction((payload: MockRuleCreate) =>
    rule ? api.patchRule(token, rule.id, payload) : api.createRule(token, payload),
  )

  const errors = validateDraft(draft)
  const countByTab = errorCountByTab(errors)
  const invalidCount = Object.keys(errors).length

  function insertTag(tag: string) {
    const next = insertAt(draft.body_template, tag, caret.current)
    set('body_template', next.text)
    caret.current = { start: next.caret, end: next.caret }
  }

  async function handleSave() {
    const invalidTab = firstInvalidTab(errors)
    if (invalidTab) {
      setTab(invalidTab)
      return
    }
    const result = await save.run(draftToPayload(draft))
    if (!result.ok) return
    toast(rule ? t('rule.toast.updated') : t('rule.toast.saved'))
    onSaved()
    onOpenChange(false)
  }

  const tabProps = { draft, set, errors }
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="w-[min(760px,94vw)]" aria-describedby={undefined}>
        <DialogHeader>{rule ? t('rule.edit.title') : t('rule.new.title')}</DialogHeader>
        <DialogBody className="space-y-5 p-0 pt-5">
          <div className="grid grid-cols-1 gap-3 px-6 sm:grid-cols-[minmax(0,1fr)_11rem_auto]">
            <TextField
              label={t('rule.field.name.label')}
              helper={t('rule.field.name.helper')}
              placeholder={t('rule.field.name.placeholder')}
              value={draft.name}
              onChange={(e) => set('name', e.target.value)}
            />
            <TextField
              label={t('rule.field.priority.label')}
              helper={t('rule.field.priority.helper')}
              type="number"
              inputMode="numeric"
              value={draft.priority}
              onChange={(e) => set('priority', Number(e.target.value))}
            />
            <SwitchField
              label={t('rule.field.enabled.label')}
              checked={draft.enabled}
              onCheckedChange={(enabled) => set('enabled', enabled)}
              fieldClassName="sm:pt-7"
            />
          </div>

          {save.error !== null && (
            <div className="px-6">
              <InlineAlert variant="danger" role="alert">
                {saveErrorMessage(save.error, rule !== null)}
              </InlineAlert>
            </div>
          )}

          <Tabs value={tab} onValueChange={(value) => setTab(value as RuleTab)}>
            <TabsList className="px-4 sm:px-5">
              {RULE_TABS.map((value) => (
                <TabsTrigger key={value} value={value}>
                  {t(TAB_LABELS[value])}
                  {countByTab[value] > 0 && (
                    <>
                      <Pill tone="danger" className="ml-1.5" aria-hidden="true">
                        {countByTab[value]}
                      </Pill>
                      <span className="sr-only">{t("rule.tab.errors", { n: countByTab[value] })}</span>
                    </>
                  )}
                </TabsTrigger>
              ))}
            </TabsList>
            <div className="px-6 py-5">
              <TabsContent value="matching">
                <MatchingTab {...tabProps} />
              </TabsContent>
              <TabsContent value="response">
                <ResponseTab {...tabProps} onCaret={(range) => (caret.current = range)} />
              </TabsContent>
              <TabsContent value="templating">
                <TemplatingTab onInsert={insertTag} />
              </TabsContent>
              <TabsContent value="actions">
                <ActionsTab {...tabProps} />
              </TabsContent>
              <TabsContent value="throttling">
                <ThrottlingTab {...tabProps} />
              </TabsContent>
            </div>
          </Tabs>
        </DialogBody>
        <DialogFooter className="flex-wrap">
          <span className="mr-auto text-caption text-text-tertiary" aria-live="polite">
            {invalidCount > 0 ? t('rule.footer.invalid', { n: invalidCount }) : t('rule.footer.ready')}
          </span>
          <Button variant="ghost" onClick={() => onOpenChange(false)}>
            {t('rule.cancel')}
          </Button>
          <Button variant="primary" loading={save.pending} onClick={() => void handleSave()}>
            {save.pending ? t('rule.saving') : t('rule.save')}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
