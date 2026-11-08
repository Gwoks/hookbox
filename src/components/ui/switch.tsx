/** Switch/toggle (design.md §3.0). Radix Switch — role=switch, visible focus,
 * on = bg-accent-fill. Picks up id / aria-describedby from a Field.
 * `SwitchField` is the labelled row (label + helper left, switch right). */
import * as SwitchPrimitive from '@radix-ui/react-switch'
import { forwardRef, useId, type ReactNode } from 'react'
import { cn } from '@/lib/cn'
import { useFieldControl } from './field'

export const Switch = forwardRef<
  React.ElementRef<typeof SwitchPrimitive.Root>,
  React.ComponentPropsWithoutRef<typeof SwitchPrimitive.Root>
>(function Switch({ className, id, 'aria-describedby': describedBy, ...props }, ref) {
  const field = useFieldControl({ id, describedBy })
  return (
    <SwitchPrimitive.Root
      ref={ref}
      id={field.id}
      aria-describedby={field.describedBy}
      className={cn(
        'peer inline-flex h-5 w-9 shrink-0 cursor-pointer items-center rounded-pill border border-transparent transition-colors duration-fast',
        'data-[state=checked]:bg-accent-fill data-[state=unchecked]:bg-surface-active',
        'disabled:cursor-not-allowed disabled:opacity-60',
        className,
      )}
      {...props}
    >
      <SwitchPrimitive.Thumb className="pointer-events-none block h-4 w-4 translate-x-0.5 rounded-pill bg-control-thumb shadow-sm transition-transform duration-fast data-[state=checked]:translate-x-[18px]" />
    </SwitchPrimitive.Root>
  )
})

export interface SwitchFieldProps
  extends Omit<React.ComponentPropsWithoutRef<typeof SwitchPrimitive.Root>, 'children'> {
  label: ReactNode
  helper?: ReactNode
  /** Classes for the row (the switch itself takes `className`). */
  fieldClassName?: string
}

/** A full-width labelled toggle row; clicking the label toggles the switch. */
export function SwitchField({ label, helper, fieldClassName, id: idProp, ...switchProps }: SwitchFieldProps) {
  const generatedId = useId()
  const id = idProp ?? generatedId
  const helperId = `${id}-desc`
  return (
    <div className={cn('flex items-start justify-between gap-4', fieldClassName)}>
      <div className="min-w-0 space-y-0.5">
        <label htmlFor={id} className="block cursor-pointer text-body-sm font-medium text-text-primary">
          {label}
        </label>
        {helper && (
          <p id={helperId} className="text-body-sm text-text-tertiary">
            {helper}
          </p>
        )}
      </div>
      <Switch id={id} aria-describedby={helper ? helperId : undefined} className="mt-0.5" {...switchProps} />
    </div>
  )
}
