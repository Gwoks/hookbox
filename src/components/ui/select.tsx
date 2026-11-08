/**
 * Select (design.md §3.0) — a NATIVE <select> styled like Input with a
 * decorative chevron, so keyboard, screen-reader and mobile pickers all work
 * for free. Picks up id / aria-describedby / invalid from a Field.
 */
import { ChevronDown } from 'lucide-react'
import { forwardRef } from 'react'
import { cn } from '@/lib/cn'
import { Field, useFieldControl, type FieldProps } from './field'
import { CONTROL_CLASSES, CONTROL_INVALID_CLASSES } from './input'

export interface SelectOption {
  value: string
  label: string
  disabled?: boolean
}

export interface SelectProps extends React.SelectHTMLAttributes<HTMLSelectElement> {
  invalid?: boolean
  /** Rendered as <option>s; alternatively pass <option> children. */
  options?: readonly SelectOption[]
}

export const Select = forwardRef<HTMLSelectElement, SelectProps>(function Select(
  { className, invalid, options, children, id, 'aria-describedby': describedBy, ...props },
  ref,
) {
  const field = useFieldControl({ id, describedBy, invalid })
  return (
    <div className="relative w-full">
      <select
        ref={ref}
        id={field.id}
        aria-describedby={field.describedBy}
        aria-invalid={field.invalid || undefined}
        className={cn(
          CONTROL_CLASSES,
          'h-9 cursor-pointer appearance-none pl-3 pr-8',
          field.invalid && CONTROL_INVALID_CLASSES,
          className,
        )}
        {...props}
      >
        {options?.map((opt) => (
          <option key={opt.value} value={opt.value} disabled={opt.disabled}>
            {opt.label}
          </option>
        ))}
        {children}
      </select>
      <ChevronDown
        className="pointer-events-none absolute right-2.5 top-1/2 h-4 w-4 -translate-y-1/2 text-text-tertiary"
        aria-hidden="true"
      />
    </div>
  )
})

/** Field + Select in one. */
export const SelectField = forwardRef<
  HTMLSelectElement,
  SelectProps & Pick<FieldProps, 'label' | 'optional' | 'helper' | 'error'> & { fieldClassName?: string }
>(function SelectField({ label, optional, helper, error, fieldClassName, id, ...selectProps }, ref) {
  return (
    <Field label={label} optional={optional} helper={helper} error={error} className={fieldClassName} id={id}>
      <Select ref={ref} {...selectProps} />
    </Field>
  )
})
