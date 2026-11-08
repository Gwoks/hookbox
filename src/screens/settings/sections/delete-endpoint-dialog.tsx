/** The typed-token delete confirm (#6): the soft-danger confirm only
 * enables once the exact token is typed; a failure stays inline. */
import { useState } from 'react'
import { api } from '@/api'
import { t } from '@/lib/copy'
import { useAsyncAction } from '@/lib/hooks/use-async-action'
import { InlineAlert } from '@/components/hookbox/inline-alert'
import { Button } from '@/components/ui/button'
import { Dialog, DialogBody, DialogContent, DialogDescription, DialogFooter, DialogHeader } from '@/components/ui/dialog'
import { TextField } from '@/components/ui/input'

export interface DeleteEndpointDialogProps {
  open: boolean
  token: string
  onClose: () => void
  onDeleted: () => void
}

export function DeleteEndpointDialog({ open, token, onClose, onDeleted }: DeleteEndpointDialogProps) {
  const [typed, setTyped] = useState('')
  const remove = useAsyncAction(() => api.deleteEndpoint(token))
  const matches = typed.trim() === token

  function handleOpenChange(next: boolean) {
    if (next || remove.pending) return
    setTyped('')
    remove.reset()
    onClose()
  }

  async function handleDelete() {
    const result = await remove.run()
    if (result.ok) onDeleted()
  }

  return (
    <Dialog open={open} onOpenChange={handleOpenChange}>
      <DialogContent>
        <DialogHeader>{t('set.confirm.delete.title', { token })}</DialogHeader>
        <DialogBody className="space-y-4">
          <DialogDescription className="text-body-sm text-text-secondary">{t('set.confirm.delete.body')}</DialogDescription>
          {remove.error !== null && (
            <InlineAlert variant="danger" role="alert">
              {t('set.error.delete')}
            </InlineAlert>
          )}
          <TextField
            label={t('set.confirm.delete.prompt')}
            mono
            autoComplete="off"
            placeholder={t('set.confirm.delete.placeholder', { token })}
            value={typed}
            onChange={(e) => setTyped(e.target.value)}
          />
        </DialogBody>
        <DialogFooter>
          <Button variant="ghost" onClick={() => handleOpenChange(false)} disabled={remove.pending}>
            {t('set.confirm.cancel')}
          </Button>
          <Button variant="danger" disabled={!matches} loading={remove.pending} onClick={() => void handleDelete()}>
            {t('set.confirm.delete.confirm')}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
