/**
 * RestoreKeyForm — sign this browser in with a saved recovery key (landing's
 * "I have a recovery key" and Settings' "Use a different recovery key").
 *
 * POST /api/session/restore {key} → the server sets the HttpOnly cookie (it
 * does NOT rotate the key) → resolve the owner's primary endpoint (creating
 * one when they have none left) → mark the tab signed in and tell the other
 * tabs (they re-check which owner the shared cookie now belongs to) → open it.
 *
 * The field is password-type with autocomplete/spellcheck off so the key
 * never lands in form history or a spellcheck service. 401 invalid_key is a
 * field error; 429 / network / other failures are a banner.
 */
import { useState, type FormEvent } from 'react'
import { useNavigate } from 'react-router-dom'
import { api, ApiError, resolvePrimaryToken, session } from '@/api'
import { signInErrorMessage } from '@/lib/api-errors'
import { t } from '@/lib/copy'
import { useAsyncAction } from '@/lib/hooks/use-async-action'
import { InlineAlert } from '@/components/hookbox/inline-alert'
import { Button } from '@/components/ui/button'
import { Field, Input } from '@/components/ui/input'

interface RestoreKeyFormProps {
  helper?: string
}

export function RestoreKeyForm({ helper }: RestoreKeyFormProps) {
  const navigate = useNavigate()
  const [key, setKey] = useState('')
  const [fieldError, setFieldError] = useState<string | null>(null)
  const [banner, setBanner] = useState<string | null>(null)

  // Mark the tab signed in only once the destination is known, so a failed
  // endpoint lookup leaves this form (and its error) on screen.
  const restore = useAsyncAction(async (candidate: string) => {
    const summary = await api.restoreSession(candidate)
    const token = await resolvePrimaryToken(summary)
    return { ownerId: summary.owner_id, token }
  })

  async function handleSubmit(event: FormEvent) {
    event.preventDefault()
    setBanner(null)
    const candidate = key.trim()
    if (!candidate) {
      setFieldError(t('recovery.restore.error.empty'))
      return
    }
    setFieldError(null)
    const result = await restore.run(candidate)
    if (result.ok) {
      setKey('')
      session.markSignedIn(result.value.ownerId, { broadcast: true })
      navigate(`/d/${result.value.token}`, { replace: true })
      return
    }
    if (result.error instanceof ApiError && result.error.status === 401) {
      setFieldError(t('recovery.restore.error.invalid'))
      return
    }
    setBanner(signInErrorMessage(result.error))
  }

  return (
    <form onSubmit={handleSubmit} className="space-y-3" noValidate>
      {banner && (
        <InlineAlert variant="danger" role="alert">
          {banner}
        </InlineAlert>
      )}
      <Field label={t('recovery.restore.label')} helper={helper ?? t('recovery.restore.helper')} error={fieldError}>
        <Input
          type="password"
          mono
          autoFocus
          autoComplete="off"
          autoCapitalize="off"
          autoCorrect="off"
          spellCheck={false}
          value={key}
          disabled={restore.pending}
          onChange={(event) => setKey(event.target.value)}
        />
      </Field>
      <Button type="submit" variant="outline" loading={restore.pending}>
        {restore.pending ? t('recovery.restore.submitting') : t('recovery.restore.submit')}
      </Button>
    </form>
  )
}
