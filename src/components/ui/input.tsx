/**
 * Text input + textarea (design.md §3.0). `mono` flag for path/target_url/
 * body_template; invalid = border-danger-fg + aria-invalid. Inside a `Field`
 * both pick up id / aria-describedby / invalid from the FieldContext.
 * `Field` and `Label` are re-exported here for the screens that import them
 * from this module.
 */
import { forwardRef } from 'react'
import { cn } from '@/lib/cn'
import { Field, useFieldControl, type FieldProps } from './field'

export { Field, Label } from './field'

/** Shared look of every text-like control (Input, Textarea, Select). */
export const CONTROL_CLASSES =
  'w-full rounded-sm border border-border-strong bg-surface text-body-sm text-text-primary placeholder:text-text-tertiary transition-colors duration-fast focus:border-accent disabled:cursor-not-allowed disabled:bg-surface-subtle disabled:text-text-tertiary'
export const CONTROL_INVALID_CLASSES = 'border-danger-fg focus:border-danger-fg'

export interface InputProps extends React.InputHTMLAttributes<HTMLInputElement> {
  invalid?: boolean
  mono?: boolean
}

export const Input = forwardRef<HTMLInputElement, InputProps>(function Input(
  { className, invalid, mono, id, 'aria-describedby': describedBy, ...props },
  ref,
) {
  const field = useFieldControl({ id, describedBy, invalid })
  return (
    <input
      ref={ref}
      id={field.id}
      aria-describedby={field.describedBy}
      aria-invalid={field.invalid || undefined}
      className={cn(
        CONTROL_CLASSES,
        'h-9 px-3',
        field.invalid && CONTROL_INVALID_CLASSES,
        mono && 'font-mono',
        className,
      )}
      {...props}
    />
  )
})

export interface TextareaProps extends React.TextareaHTMLAttributes<HTMLTextAreaElement> {
  invalid?: boolean
  mono?: boolean
}

export const Textarea = forwardRef<HTMLTextAreaElement, TextareaProps>(function Textarea(
  { className, invalid, mono, id, 'aria-describedby': describedBy, ...props },
  ref,
) {
  const field = useFieldControl({ id, describedBy, invalid })
  return (
    <textarea
      ref={ref}
      id={field.id}
      aria-describedby={field.describedBy}
      aria-invalid={field.invalid || undefined}
      className={cn(
        CONTROL_CLASSES,
        'px-3 py-2',
        field.invalid && CONTROL_INVALID_CLASSES,
        mono && 'font-mono',
        className,
      )}
      {...props}
    />
  )
})

type FieldChrome = Pick<FieldProps, 'label' | 'optional' | 'helper' | 'error'> & {
  /** Classes for the wrapping Field (the input itself takes `className`). */
  fieldClassName?: string
}

/** Field + Input in one: `<TextField label="Name" value={…} onChange={…} />`. */
export const TextField = forwardRef<HTMLInputElement, InputProps & FieldChrome>(function TextField(
  { label, optional, helper, error, fieldClassName, id, ...inputProps },
  ref,
) {
  return (
    <Field label={label} optional={optional} helper={helper} error={error} className={fieldClassName} id={id}>
      <Input ref={ref} {...inputProps} />
    </Field>
  )
})
