/**
 * useConfigImport — F3 import as one state machine:
 * idle → (file read + fully validated, AC-16: nothing is written yet) →
 * confirming (the AC-S21 diff) → applying (one PATCH, then one POST per
 * rule in order; stop at the first failure, no rollback, add-never-replace —
 * AC-17/18/19) → report.
 */
import { useState, type ChangeEvent } from 'react'
import { api, ApiError, type EndpointDetail } from '@/api'
import { t } from '@/lib/copy'
import {
  computeConfigDiff,
  MAX_BUNDLE_BYTES,
  parseBundle,
  toBundleEndpoint,
  type ConfigBundle,
  type ConfigDiffRow,
} from '@/lib/config-bundle'
import { useToast } from '@/components/ui/toast'

export type ImportPhase =
  | { kind: 'idle' }
  | { kind: 'confirming'; bundle: ConfigBundle; diff: ConfigDiffRow[]; existingRuleCount: number }
  | { kind: 'applying'; label: string; value: number; max: number }
  | { kind: 'report'; variant: 'success' | 'danger'; message: string; addedRules: number }

const IDLE: ImportPhase = { kind: 'idle' }

const failureDetail = (err: unknown) => (err instanceof ApiError ? err.message : t('common.error.generic'))

interface Options {
  token: string
  endpoint: EndpointDetail
  onImported: (endpoint: EndpointDetail) => void
}

export function useConfigImport({ token, endpoint, onImported }: Options) {
  const { toast } = useToast()
  const [phase, setPhase] = useState<ImportPhase>(IDLE)
  const [fileError, setFileError] = useState<string | null>(null)

  async function readBundle(file: File) {
    const result = parseBundle(await file.text())
    if (!result.success) {
      setFileError(result.message)
      return
    }
    // Best effort: the confirm still shows a count if this read fails.
    const existingRuleCount = await api
      .listRules(token)
      .then((rules) => rules.length)
      .catch(() => 0)
    const diff = computeConfigDiff(toBundleEndpoint(endpoint), result.bundle.endpoint)
    setPhase({ kind: 'confirming', bundle: result.bundle, diff, existingRuleCount })
  }

  function handleFileChange(e: ChangeEvent<HTMLInputElement>) {
    const file = e.target.files?.[0]
    // AC-90: reset so re-selecting the SAME file after a rejection fires.
    e.target.value = ''
    if (!file) return // a cancelled file dialog is a silent no-op
    setFileError(null)
    setPhase(IDLE)
    if (file.size === 0) return setFileError(t('set.config.import.invalid.empty'))
    if (file.size > MAX_BUNDLE_BYTES) return setFileError(t('set.config.import.tooLarge'))
    readBundle(file).catch(() => setFileError(t('set.config.import.invalid.json')))
  }

  async function createRules(bundle: ConfigBundle, updated: EndpointDetail): Promise<boolean> {
    const total = bundle.rules.length
    for (let i = 0; i < total; i++) {
      const label = t('set.config.import.progressRules', { i: i + 1, n: total })
      setPhase({ kind: 'applying', label, value: i, max: total })
      try {
        await api.createRule(token, bundle.rules[i])
      } catch (err) {
        onImported(updated) // AC-19: the config step DID apply
        const name = bundle.rules[i].name || t('rules.row.unnamed')
        const message = t('set.config.import.failedRule', {
          done: i,
          total,
          index: i + 1,
          name,
          detail: failureDetail(err),
        })
        setPhase({ kind: 'report', variant: 'danger', message, addedRules: i })
        return false
      }
    }
    return true
  }

  async function runImport(bundle: ConfigBundle) {
    const total = bundle.rules.length
    setPhase({ kind: 'applying', label: t('set.config.import.progressConfig'), value: 0, max: total })
    let updated: EndpointDetail
    try {
      updated = await api.patchEndpoint(token, bundle.endpoint)
    } catch (err) {
      const message = t('set.config.import.failedConfig', { detail: failureDetail(err) })
      setPhase({ kind: 'report', variant: 'danger', message, addedRules: 0 })
      return
    }
    if (!(await createRules(bundle, updated))) return
    onImported(updated)
    toast(t('set.config.toast.imported'))
    const message = total > 0 ? t('set.config.import.done', { n: total }) : t('set.config.import.done.noRules')
    setPhase({ kind: 'report', variant: 'success', message, addedRules: total })
  }

  return {
    phase,
    fileError,
    isApplying: phase.kind === 'applying',
    handleFileChange,
    /** ConfirmDialog's onConfirm: close at once, progress shows inline. */
    confirm: async () => {
      if (phase.kind === 'confirming') void runImport(phase.bundle)
    },
    reset: () => setPhase(IDLE),
    /** Closing the confirm must not clobber an import it just started. */
    closeConfirm: () => setPhase((prev) => (prev.kind === 'confirming' ? IDLE : prev)),
  }
}
