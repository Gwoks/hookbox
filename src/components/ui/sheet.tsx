/**
 * Sheet — a Radix Dialog docked to an edge: `right` (mobile nav "Menu") or
 * `bottom` (the dashboard inspector below md). Modal, focus-trapped,
 * Escape/scrim to close, a required title for its accessible name. Floating
 * surface, so it keeps a shadow (DESIGN.md allows shadows on overlays only).
 */
import * as DialogPrimitive from '@radix-ui/react-dialog'
import { X } from 'lucide-react'
import { forwardRef, type ReactNode } from 'react'
import { cn } from '@/lib/cn'
import { t } from '@/lib/copy'

export const Sheet = DialogPrimitive.Root
export const SheetTrigger = DialogPrimitive.Trigger
export const SheetClose = DialogPrimitive.Close

const SIDE_CLASSES = {
  right: 'inset-y-0 right-0 h-full w-[min(360px,88vw)] border-l animate-sheet-in-right',
  bottom: 'inset-x-0 bottom-0 max-h-[85vh] rounded-t-md border-t animate-sheet-in',
} as const

export interface SheetContentProps
  extends Omit<React.ComponentPropsWithoutRef<typeof DialogPrimitive.Content>, 'title'> {
  title: ReactNode
  description?: ReactNode
  side?: keyof typeof SIDE_CLASSES
}

export const SheetContent = forwardRef<React.ElementRef<typeof DialogPrimitive.Content>, SheetContentProps>(
  function SheetContent({ title, description, side = 'right', className, children, ...props }, ref) {
    return (
      <DialogPrimitive.Portal>
        <DialogPrimitive.Overlay
          className="fixed inset-0 z-dialog animate-overlay-in"
          style={{ background: 'var(--overlay-scrim)' }}
        />
        <DialogPrimitive.Content
          ref={ref}
          // Radix wires a Description automatically; without one it wants an
          // explicit `aria-describedby={undefined}` (no dangling reference).
          {...(description ? {} : { 'aria-describedby': undefined })}
          className={cn(
            'fixed z-dialog flex flex-col border-border bg-surface shadow-lg',
            SIDE_CLASSES[side],
            className,
          )}
          {...props}
        >
          <div className="flex items-start justify-between gap-3 border-b border-border px-5 py-4">
            <div className="min-w-0 space-y-1">
              <DialogPrimitive.Title className="text-h4 text-text-primary">{title}</DialogPrimitive.Title>
              {description && (
                <DialogPrimitive.Description className="text-body-sm text-text-secondary">
                  {description}
                </DialogPrimitive.Description>
              )}
            </div>
            <DialogPrimitive.Close
              className="rounded-sm p-1 text-text-tertiary hover:bg-surface-hover hover:text-text-primary"
              aria-label={t('common.close.aria')}
            >
              <X className="h-4 w-4" aria-hidden="true" />
            </DialogPrimitive.Close>
          </div>
          <div className="min-h-0 flex-1 overflow-y-auto px-5 py-4">{children}</div>
        </DialogPrimitive.Content>
      </DialogPrimitive.Portal>
    )
  },
)
