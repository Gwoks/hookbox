/** Danger zone: a danger-bordered section whose one action, Delete
 * endpoint, is the SOFT danger button (no filled red anywhere, D3). On
 * success the tab goes back to the landing, which opens (or creates) the
 * owner's next endpoint. */
import { Trash2 } from 'lucide-react'
import { useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { t } from '@/lib/copy'
import { Button } from '@/components/ui/button'
import { SettingsSection } from '@/components/ui/section'
import { useToast } from '@/components/ui/toast'
import { DeleteEndpointDialog } from './delete-endpoint-dialog'

export function DangerZoneSection({ token }: { token: string }) {
  const navigate = useNavigate()
  const { toast } = useToast()
  const [isConfirming, setIsConfirming] = useState(false)

  return (
    <SettingsSection id="danger-zone" title={t('set.danger.title')} tone="danger">
      <div className="flex flex-wrap items-center justify-between gap-4">
        <div className="min-w-0 space-y-0.5">
          <p className="text-body-sm font-medium text-text-primary">{t('set.danger.delete.label')}</p>
          <p className="text-body-sm text-text-tertiary">{t('set.danger.delete.helper')}</p>
        </div>
        <Button variant="danger" onClick={() => setIsConfirming(true)}>
          <Trash2 className="h-3.5 w-3.5" aria-hidden="true" />
          {t('set.danger.delete.label')}
        </Button>
      </div>
      <DeleteEndpointDialog
        open={isConfirming}
        token={token}
        onClose={() => setIsConfirming(false)}
        onDeleted={() => {
          toast(t('set.toast.deleted'))
          navigate('/', { replace: true })
        }}
      />
    </SettingsSection>
  )
}
