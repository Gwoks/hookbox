/** Dialog (design.md §3.0/§3.10). bg-surface, radius-md, shadow-lg, scrim;
 * mobile → full-screen sheet. Radix Dialog re-themed by tokens only. */
import * as DialogPrimitive from '@radix-ui/react-dialog'
import { X } from 'lucide-react'
import { forwardRef } from 'react'
import { cn } from '@/lib/cn'

export const Dialog = DialogPrimitive.Root
export const DialogTrigger = DialogPrimitive.Trigger
export const DialogClose = DialogPrimitive.Close

export const DialogContent = forwardRef<
  React.ElementRef<typeof DialogPrimitive.Content>,
  React.ComponentPropsWithoutRef<typeof DialogPrimitive.Content> & { hideClose?: boolean }
>(function DialogContent({ className, children, hideClose, ...props }, ref) {
  return (
    <DialogPrimitive.Portal>
      <DialogPrimitive.Overlay
        className="fixed inset-0 z-dialog animate-overlay-in"
        style={{ background: 'var(--overlay-scrim)' }}
      />
      <DialogPrimitive.Content
        ref={ref}
        className={cn(
          'fixed left-1/2 top-1/2 z-dialog flex max-h-[90vh] w-[min(560px,92vw)] -translate-x-1/2 -translate-y-1/2 flex-col',
          'rounded-md border border-border bg-surface shadow-lg animate-content-in',
          'max-sm:left-0 max-sm:top-auto max-sm:bottom-0 max-sm:max-h-[92vh] max-sm:w-screen max-sm:translate-x-0 max-sm:translate-y-0 max-sm:rounded-b-none max-sm:animate-sheet-in',
          className,
        )}
        {...props}
      >
        {children}
        {!hideClose && (
          <DialogPrimitive.Close
            className="absolute right-4 top-4 rounded-sm p-1 text-text-tertiary hover:bg-surface-hover hover:text-text-primary"
            aria-label="Close"
          >
            <X className="h-4 w-4" aria-hidden="true" />
          </DialogPrimitive.Close>
        )}
      </DialogPrimitive.Content>
    </DialogPrimitive.Portal>
  )
})

export function DialogHeader({ className, children }: { className?: string; children: React.ReactNode }) {
  return (
    <div className={cn('border-b border-border px-6 py-4', className)}>
      <DialogPrimitive.Title className="text-h2 text-text-primary">{children}</DialogPrimitive.Title>
    </div>
  )
}

export function DialogBody({ className, children }: { className?: string; children: React.ReactNode }) {
  return <div className={cn('flex-1 overflow-y-auto px-6 py-5', className)}>{children}</div>
}

export function DialogFooter({ className, children }: { className?: string; children: React.ReactNode }) {
  return (
    <div className={cn('flex items-center justify-end gap-2 border-t border-border px-6 py-4', className)}>
      {children}
    </div>
  )
}

export const DialogDescription = DialogPrimitive.Description
