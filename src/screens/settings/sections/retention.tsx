/** Retention & state: clear request history (#14) and clear endpoint state
 * (#16), each behind the shared ConfirmDialog (failures shown inline). */
import { useState } from 'react'
import { api } from '@/api'
import { t } from '@/lib/copy'
import { ConfirmDialog } from '@/components/hookbox/confirm-dialog'
import { Button } from '@/components/ui/button'
import { SettingsSection } from '@/components/ui/section'
import { useToast } from '@/components/ui/toast'

export function RetentionSection({ token }: { token: string }) {
  const { toast } = useToast()
  const [confirm, setConfirm] = useState<'history' | 'state' | null>(null)
  const close = () => setConfirm(null)

  return (
    <SettingsSection id="retention" title={t('set.retention.title')} description={t('set.retention.note')}>
      <div className="flex flex-wrap gap-2">
        <Button variant="outline" onClick={() => setConfirm('history')}>
          {t('set.retention.clearHistory')}
        </Button>
        <Button variant="outline" onClick={() => setConfirm('state')}>
          {t('set.retention.clearState')}
        </Button>
      </div>
      <p className="text-caption text-text-tertiary">{t('set.retention.stateNote')}</p>

      <ConfirmDialog
        open={confirm === 'history'}
        onClose={close}
        title={t('set.confirm.clearHistory.title')}
        body={t('set.confirm.clearHistory.body')}
        confirmLabel={t('set.confirm.clearHistory.confirm')}
        onConfirm={async () => {
          await api.clearRequests(token)
          toast(t('set.toast.historyCleared'))
        }}
      />
      <ConfirmDialog
        open={confirm === 'state'}
        onClose={close}
        title={t('set.confirm.clearState.title')}
        body={t('set.confirm.clearState.body')}
        confirmLabel={t('set.confirm.clearState.confirm')}
        onConfirm={async () => {
          await api.clearState(token)
          toast(t('set.toast.stateCleared'))
        }}
      />
    </SettingsSection>
  )
}
