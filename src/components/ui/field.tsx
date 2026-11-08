/**
 * Field system (design.md §3.0, §9). `Field` renders a label + helper/error
 * and provides a FieldContext (id, describedBy, invalid) that Input,
 * Textarea, Select, Switch and Slider consume automatically — so a control
 * placed inside a Field is labelled, described and marked invalid without
 * any prop threading. Errors are text + aria-invalid, never color only.
 *
 * Explicit props on a control always win over the context. The `render`
 * prop is the legacy API (every screen still uses it) and is removed in S6.
 */
import { createContext, useContext, useId, type ReactNode } from 'react'
import { cn } from '@/lib/cn'

export interface FieldControlState {
  id: string
  describedBy?: string
  invalid: boolean
}

export const FieldContext = createContext<FieldControlState | null>(null)

/** Merge a control's explicit a11y props with the surrounding Field's. */
export function useFieldControl(explicit: {
  id?: string
  describedBy?: string
  invalid?: boolean
}): { id?: string; describedBy?: string; invalid: boolean } {
  const field = useContext(FieldContext)
  return {
    id: explicit.id ?? field?.id,
    describedBy: explicit.describedBy ?? field?.describedBy,
    invalid: explicit.invalid ?? field?.invalid ?? false,
  }
}

export function Label({
  className,
  children,
  htmlFor,
  id,
  optional,
}: {
  className?: string
  children: ReactNode
  htmlFor?: string
  id?: string
  optional?: boolean
}) {
  return (
    <label
      id={id}
      htmlFor={htmlFor}
      className={cn('block text-body-sm font-medium text-text-primary', className)}
    >
      {children}
      {optional && <span className="ml-1 font-normal text-text-tertiary">(optional)</span>}
    </label>
  )
}

/** Helper text, or the error (role="alert") when there is one. */
export function FieldMessage({
  id,
  helper,
  error,
}: {
  id: string
  helper?: ReactNode
  error?: string | null
}) {
  if (error) {
    return (
      <p id={id} className="text-body-sm text-danger-fg" role="alert">
        {error}
      </p>
    )
  }
  if (!helper) return null
  return (
    <p id={id} className="text-body-sm text-text-tertiary">
      {helper}
    </p>
  )
}

export interface FieldProps {
  label: ReactNode
  optional?: boolean
  helper?: ReactNode
  error?: string | null
  className?: string
  /** Override the generated control id. */
  id?: string
  children?: ReactNode
  /** @deprecated Pass the control as `children` (removed in S6). */
  render?: (props: FieldControlState) => ReactNode
}

export function Field({ label, optional, helper, error, className, id: idProp, children, render }: FieldProps) {
  const generatedId = useId()
  const id = idProp ?? generatedId
  const messageId = `${id}-desc`
  const state: FieldControlState = {
    id,
    describedBy: helper || error ? messageId : undefined,
    invalid: !!error,
  }
  return (
    <FieldContext.Provider value={state}>
      <div className={cn('space-y-1.5', className)}>
        <Label htmlFor={id} optional={optional}>
          {label}
        </Label>
        {render ? render(state) : children}
        <FieldMessage id={messageId} helper={helper} error={error} />
      </div>
    </FieldContext.Provider>
  )
}

/** A titled group of related fields (legend + optional helper). Resets the
 * FieldContext so nested controls never inherit an outer Field's id. */
export function Fieldset({
  legend,
  helper,
  className,
  children,
}: {
  legend: ReactNode
  helper?: ReactNode
  className?: string
  children: ReactNode
}) {
  const helperId = useId()
  return (
    <fieldset className={cn('min-w-0 space-y-3', className)} aria-describedby={helper ? helperId : undefined}>
      <legend className="text-h4 text-text-primary">{legend}</legend>
      {helper && (
        <p id={helperId} className="-mt-1 text-body-sm text-text-tertiary">
          {helper}
        </p>
      )}
      <FieldContext.Provider value={null}>{children}</FieldContext.Provider>
    </fieldset>
  )
}
