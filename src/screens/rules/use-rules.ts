/**
 * useRules — the rules list (GET, server order) and every list mutation:
 * the optimistic enable toggle (reverted on failure), duplicate, delete,
 * and F6's "Add default rule", which re-checks the list right before
 * creating (AC-123c: another tab may already have added a catch-all, and
 * there is no server-side uniqueness guard).
 */
import { useCallback } from 'react'
import { z } from 'zod'
import { api, mockRuleSchema, type MockRule } from '@/api'
import { t } from '@/lib/copy'
import { useAsyncAction } from '@/lib/hooks/use-async-action'
import { useResource } from '@/lib/hooks/use-resource'
import { useToast } from '@/components/ui/toast'
import { DEFAULT_CATCH_ALL_RULE, hasCatchAllRule } from './default-rule'

const rulesSchema = z.array(mockRuleSchema)

/** Re-parse to the precise output type: the client's generic infers the
 * looser input type for schemas carrying zod defaults. */
async function fetchRules(token: string, signal?: AbortSignal): Promise<MockRule[]> {
  return rulesSchema.parse(await api.listRules(token, { signal }))
}

function duplicatePayload(rule: MockRule) {
  return {
    name: rule.name ? `${rule.name} (copy)` : null,
    priority: rule.priority,
    enabled: rule.enabled,
    match: rule.match,
    response: rule.response,
    state_writes: rule.state_writes,
    latency_ms: rule.latency_ms,
    rate_limit_per_min: rule.rate_limit_per_min,
    chaos_mode: rule.chaos_mode,
    webhook_action: rule.webhook_action,
  }
}

export function useRules(token: string) {
  const { toast } = useToast()
  const list = useResource((signal) => fetchRules(token, signal), [token])
  const { reload, setData } = list

  const refresh = useCallback(() => reload({ silent: true }), [reload])

  async function toggle(rule: MockRule, enabled: boolean) {
    const setEnabled = (value: boolean) =>
      setData((prev) => (prev ?? []).map((r) => (r.id === rule.id ? { ...r, enabled: value } : r)))
    setEnabled(enabled)
    try {
      await api.patchRule(token, rule.id, { enabled })
      toast(enabled ? t('rules.toast.enabled') : t('rules.toast.disabled'))
    } catch {
      setEnabled(!enabled)
      toast(t('rules.error.toggle'), 'danger')
    }
  }

  async function duplicate(rule: MockRule) {
    try {
      await api.createRule(token, duplicatePayload(rule))
      toast(t('rules.toast.duplicated'))
      void refresh()
    } catch {
      toast(t('rule.error.save'), 'danger')
    }
  }

  /** For ConfirmDialog: a rejection is shown inline there. */
  async function remove(rule: MockRule) {
    await api.deleteRule(token, rule.id)
    toast(t('rules.toast.deleted'))
    void refresh()
  }

  /** Throws on failure (the shadow confirm renders it inline). */
  const createDefault = useCallback(async () => {
    const fresh = await fetchRules(token)
    setData(fresh)
    if (hasCatchAllRule(fresh)) {
      toast(t('rules.default.error.duplicate'), 'danger')
      return
    }
    await api.createRule(token, DEFAULT_CATCH_ALL_RULE)
    toast(t('rules.default.toast'))
    void refresh()
  }, [refresh, setData, toast, token])

  const addDefault = useAsyncAction(createDefault)

  /** The direct (no-confirm) path: failures become a toast. */
  async function addDefaultDirect() {
    const result = await addDefault.run()
    if (!result.ok) toast(t('rules.default.error'), 'danger')
  }

  /** The shadow confirm's onConfirm: rethrows so the dialog shows it. */
  async function addDefaultConfirmed() {
    const result = await addDefault.run()
    if (!result.ok) throw result.error
  }

  return {
    list,
    refresh,
    toggle,
    duplicate,
    remove,
    isAddingDefault: addDefault.pending,
    addDefaultDirect,
    addDefaultConfirmed,
  }
}
