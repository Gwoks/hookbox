/**
 * useSettingsForm — the settings draft (useDraft, re-seeded whenever the
 * shared endpoint is replaced: a Save or an import bumps `revision`, AC-89a),
 * validation shown after a blur or a Save attempt, and Save itself (PATCH
 * #5 → `replace()` the shared endpoint). Discard re-seeds from the server.
 */
import { useCallback, useMemo, useState } from 'react'
import { api, type EndpointConfigPatch } from '@/api'
import { t } from '@/lib/copy'
import { useAsyncAction } from '@/lib/hooks/use-async-action'
import { useDraft } from '@/lib/hooks/use-draft'
import { useEndpoints } from '@/components/owner/endpoint-context'
import { useToast } from '@/components/ui/toast'
import { formFromEndpoint, isFormDirty, toPatch, validateSettings, type SettingsErrors, type SettingsForm } from './form'

export interface SettingsFormState {
  draft: SettingsForm
  set: <K extends keyof SettingsForm>(key: K, value: SettingsForm[K]) => void
  errors: SettingsErrors
  revealErrors: () => void
  isDirty: boolean
  isSaving: boolean
  saveError: string | null
  submit: () => Promise<void>
  discard: () => void
}

export function useSettingsForm(): SettingsFormState {
  const { token, endpoint, revision, replace } = useEndpoints()
  const { toast } = useToast()
  const base = useMemo(() => formFromEndpoint(endpoint), [endpoint])
  const { draft, set, reset } = useDraft(base, { resetKey: revision })
  const [isShowingErrors, setIsShowingErrors] = useState(false)
  const save = useAsyncAction((patch: EndpointConfigPatch) => api.patchEndpoint(token, patch))
  const { run: runSave, reset: resetSave } = save

  const errors = validateSettings(draft)
  const hasErrors = Object.keys(errors).length > 0

  const submit = useCallback(async () => {
    setIsShowingErrors(true)
    if (hasErrors) return
    const result = await runSave(toPatch(draft))
    if (!result.ok) return
    replace(result.value)
    toast(t('set.toast.saved'))
  }, [draft, hasErrors, replace, runSave, toast])

  const discard = useCallback(() => {
    reset()
    resetSave()
    setIsShowingErrors(false)
  }, [reset, resetSave])

  return {
    draft,
    set,
    errors: isShowingErrors ? errors : {},
    revealErrors: () => setIsShowingErrors(true),
    isDirty: isFormDirty(base, draft),
    isSaving: save.pending,
    saveError: save.error !== null ? t('set.error.save') : null,
    submit,
    discard,
  }
}
