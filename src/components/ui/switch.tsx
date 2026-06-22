/** Switch/toggle (design.md §3.0). Radix Switch — role=switch, visible focus,
 * on = bg-accent-fill. Used for Auto-CRUD, CORS, rule `enabled`. */
import * as SwitchPrimitive from '@radix-ui/react-switch'
import { forwardRef } from 'react'
import { cn } from '@/lib/cn'

export const Switch = forwardRef<
  React.ElementRef<typeof SwitchPrimitive.Root>,
  React.ComponentPropsWithoutRef<typeof SwitchPrimitive.Root>
>(function Switch({ className, ...props }, ref) {
  return (
    <SwitchPrimitive.Root
      ref={ref}
      className={cn(
        'peer inline-flex h-5 w-9 shrink-0 cursor-pointer items-center rounded-pill border border-transparent transition-colors duration-fast',
        'data-[state=checked]:bg-accent-fill data-[state=unchecked]:bg-surface-active',
        className,
      )}
      {...props}
    >
      <SwitchPrimitive.Thumb className="pointer-events-none block h-4 w-4 translate-x-0.5 rounded-full bg-white shadow-sm transition-transform duration-fast data-[state=checked]:translate-x-[18px]" />
    </SwitchPrimitive.Root>
  )
})
