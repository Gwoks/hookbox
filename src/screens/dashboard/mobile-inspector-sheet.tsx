/** Below `md` the inspector has no pane of its own: selecting a feed row
 * opens it in a bottom Sheet; closing the sheet clears the selection. */
import { t } from '@/lib/copy'
import { Sheet, SheetContent } from '@/components/ui/sheet'
import { Inspector } from './inspector'

export function MobileInspectorSheet({
  id,
  isLive,
  onClose,
}: {
  id: number | null
  isLive: boolean
  onClose: () => void
}) {
  return (
    <Sheet open={id !== null} onOpenChange={(open) => !open && onClose()}>
      <SheetContent side="bottom" title={t('insp.sheet.title')}>
        <div className="-mx-5 -my-4">
          <Inspector id={id} isLive={isLive} />
        </div>
      </SheetContent>
    </Sheet>
  )
}
