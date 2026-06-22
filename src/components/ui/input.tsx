/**
 * Text input + textarea + labeled field (design.md §3.0). `mono` flag for
 * path/target_url/body_template; error = border-danger-fg + message +
 * aria-invalid + aria-describedby (text, not color-only — §9). Mirrors the
 * reference input.tsx.
 */
import { forwardRef, useId } from 'react'
import { cn } from '@/lib/cn'

export interface InputProps extends React.InputHTMLAttributes<HTMLInputElement> {
  invalid?: boolean
  mono?: boolean
}

export const Input = forwardRef<HTMLInputElement, InputProps>(function Input(
  { className, invalid, mono, ...props },
  ref,
) {
  return (
    <input
      ref={ref}
      aria-invalid={invalid || undefined}
      className={cn(
        'h-9 w-full rounded-sm border border-border-strong bg-surface px-3 text-body-sm text-text-primary placeholder:text-text-tertiary',
        'transition-colors duration-fast focus:border-accent disabled:cursor-not-allowed disabled:bg-surface-subtle disabled:text-text-tertiary',
        invalid && 'border-danger-fg focus:border-danger-fg',
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
  { className, invalid, mono, ...props },
  ref,
) {
  return (
    <textarea
      ref={ref}
      aria-invalid={invalid || undefined}
      className={cn(
        'w-full rounded-sm border border-border-strong bg-surface px-3 py-2 text-body-sm text-text-primary placeholder:text-text-tertiary',
        'transition-colors duration-fast focus:border-accent disabled:cursor-not-allowed disabled:bg-surface-subtle',
        invalid && 'border-danger-fg focus:border-danger-fg',
        mono && 'font-mono',
        className,
      )}
      {...props}
    />
  )
})

export function Label({
  className,
  children,
  htmlFor,
  optional,
}: {
  className?: string
  children: React.ReactNode
  htmlFor?: string
  optional?: boolean
}) {
  return (
    <label
      htmlFor={htmlFor}
      className={cn('block text-body-sm font-medium text-text-primary', className)}
    >
      {children}
      {optional && <span className="ml-1 font-normal text-text-tertiary">(optional)</span>}
    </label>
  )
}

export function Field({
  label,
  optional,
  helper,
  error,
  className,
  render,
}: {
  label: string
  optional?: boolean
  helper?: React.ReactNode
  error?: string | null
  className?: string
  render: (props: { id: string; describedBy?: string; invalid: boolean }) => React.ReactNode
}) {
  const id = useId()
  const describeId = `${id}-desc`
  const invalid = !!error
  return (
    <div className={cn('space-y-1.5', className)}>
      <Label htmlFor={id} optional={optional}>
        {label}
      </Label>
      {render({ id, describedBy: helper || error ? describeId : undefined, invalid })}
      {error ? (
        <p id={describeId} className="text-body-sm text-danger-fg" role="alert">
          {error}
        </p>
      ) : helper ? (
        <p id={describeId} className="text-body-sm text-text-tertiary">
          {helper}
        </p>
      ) : null}
    </div>
  )
}
