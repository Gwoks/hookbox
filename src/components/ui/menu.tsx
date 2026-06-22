/** DropdownMenu (design.md §3.0). Radix DropdownMenu re-themed by tokens.
 * Used by the mobile overflow / endpoint switcher / account menu. */
import * as MenuPrimitive from '@radix-ui/react-dropdown-menu'
import { forwardRef } from 'react'
import { cn } from '@/lib/cn'

export const Menu = MenuPrimitive.Root
export const MenuTrigger = MenuPrimitive.Trigger

export const MenuContent = forwardRef<
  React.ElementRef<typeof MenuPrimitive.Content>,
  React.ComponentPropsWithoutRef<typeof MenuPrimitive.Content>
>(function MenuContent({ className, sideOffset = 6, ...props }, ref) {
  return (
    <MenuPrimitive.Portal>
      <MenuPrimitive.Content
        ref={ref}
        sideOffset={sideOffset}
        className={cn(
          'z-dropdown min-w-[180px] rounded-md border border-border bg-surface-raised p-1 shadow-md animate-content-in',
          className,
        )}
        {...props}
      />
    </MenuPrimitive.Portal>
  )
})

export const MenuItem = forwardRef<
  React.ElementRef<typeof MenuPrimitive.Item>,
  React.ComponentPropsWithoutRef<typeof MenuPrimitive.Item>
>(function MenuItem({ className, ...props }, ref) {
  return (
    <MenuPrimitive.Item
      ref={ref}
      className={cn(
        'flex cursor-pointer items-center gap-2 rounded-sm px-2 py-1.5 text-body-sm text-text-secondary outline-none',
        'focus:bg-surface-hover focus:text-text-primary data-[disabled]:cursor-not-allowed data-[disabled]:text-text-tertiary',
        className,
      )}
      {...props}
    />
  )
})

export const MenuSeparator = forwardRef<
  React.ElementRef<typeof MenuPrimitive.Separator>,
  React.ComponentPropsWithoutRef<typeof MenuPrimitive.Separator>
>(function MenuSeparator({ className, ...props }, ref) {
  return <MenuPrimitive.Separator ref={ref} className={cn('my-1 h-px bg-border', className)} {...props} />
})
