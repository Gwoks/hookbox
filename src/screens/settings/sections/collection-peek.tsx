/** Auto-CRUD collection peek (#17) and clear (#18, now behind a confirm).
 * Names follow the server's rule: letters, numbers, _ and -, max 64. */
import { useState } from 'react'
import { api } from '@/api'
import { t } from '@/lib/copy'
import { useAsyncAction } from '@/lib/hooks/use-async-action'
import { ConfirmDialog } from '@/components/hookbox/confirm-dialog'
import { JsonTree } from '@/components/hookbox/json-tree'
import { Button } from '@/components/ui/button'
import { Field } from '@/components/ui/field'
import { Input } from '@/components/ui/input'
import { useToast } from '@/components/ui/toast'

const COLLECTION_RE = /^[A-Za-z0-9_-]{1,64}$/

interface Peeked {
  name: string
  items: Record<string, unknown>[]
}

export function CollectionPeek({ token }: { token: string }) {
  const { toast } = useToast()
  const [nameInput, setNameInput] = useState('')
  const [peeked, setPeeked] = useState<Peeked | null>(null)
  const [validationError, setValidationError] = useState<string | null>(null)
  const [confirmClearOpen, setConfirmClearOpen] = useState(false)
  const peek = useAsyncAction((name: string) => api.peekCollection(token, name))
  const error = validationError ?? (peek.error !== null ? t('set.crud.peek.invalid') : null)

  async function handlePeek() {
    const name = nameInput.trim()
    if (!COLLECTION_RE.test(name)) {
      setValidationError(t('set.crud.peek.invalid'))
      return
    }
    setValidationError(null)
    const result = await peek.run(name)
    if (result.ok) setPeeked({ name, items: result.value.items })
  }

  async function clearCollection() {
    if (!peeked) return
    await api.clearCollection(token, peeked.name)
    toast(t('set.toast.collectionCleared'))
    setPeeked({ ...peeked, items: [] })
  }

  return (
    <div className="space-y-3">
      <Field label={t('set.crud.peek.label')} error={error}>
        <div className="flex items-center gap-2">
          <Input
            mono
            placeholder={t('set.crud.peek.placeholder')}
            value={nameInput}
            onChange={(e) => setNameInput(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && void handlePeek()}
          />
          <Button variant="outline" onClick={() => void handlePeek()} loading={peek.pending}>
            {t('set.crud.peek.view')}
          </Button>
        </div>
      </Field>
      {peeked && (
        <div className="space-y-2">
          <div className="flex items-center justify-between gap-2">
            <span className="text-caption text-text-tertiary">{t('set.crud.peek.count', { n: peeked.items.length })}</span>
            <Button variant="danger-ghost" size="sm" onClick={() => setConfirmClearOpen(true)}>
              {t('set.crud.peek.clear')}
            </Button>
          </div>
          {peeked.items.length === 0 ? (
            <p className="text-body-sm text-text-tertiary">{t('set.crud.peek.empty')}</p>
          ) : (
            <JsonTree raw={JSON.stringify(peeked.items)} />
          )}
        </div>
      )}
      <ConfirmDialog
        open={confirmClearOpen}
        onClose={() => setConfirmClearOpen(false)}
        title={t('set.confirm.clearCollection.title', { name: peeked?.name ?? '' })}
        body={t('set.confirm.clearCollection.body')}
        confirmLabel={t('set.confirm.clearCollection.confirm')}
        onConfirm={clearCollection}
      />
    </div>
  )
}
