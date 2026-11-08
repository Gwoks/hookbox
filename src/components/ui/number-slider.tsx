/**
 * NumberSlider — a labelled range slider paired with a number input (latency,
 * rate limit, chaos %). Replaces the two private `NumberWithSlider` copies in
 * settings / rule-builder, fixing their a11y: the visible label is a real
 * <label htmlFor> of the number input and labels the slider through
 * aria-labelledby; unit + helper describe the input. Typed values clamp into
 * [min, max]. `nullable` adds an "Inherit" switch: on → the value is `null`
 * (the rule inherits the endpoint setting) and both controls are disabled;
 * off → restores the last concrete value.
 */
import { useId, useState, type ReactNode } from 'react'
import { cn } from '@/lib/cn'
import { t } from '@/lib/copy'
import { FieldContext, Label } from './field'
import { Input } from './input'
import { Slider } from './slider'
import { Switch } from './switch'

interface NumberSliderBase {
  label: string
  unit?: string
  helper?: ReactNode
  min: number
  max: number
  step?: number
  className?: string
}

export type NumberSliderProps = NumberSliderBase &
  (
    | { nullable?: false; value: number; onChange: (value: number) => void }
    | { nullable: true; value: number | null; onChange: (value: number | null) => void }
  )

const clampTo = (min: number, max: number) => (v: number) => Math.min(max, Math.max(min, v))

export function NumberSlider(props: NumberSliderProps) {
  const { label, unit, helper, min, max, step = 1, className } = props
  const id = useId()
  const ids = { label: `${id}-label`, input: `${id}-input`, unit: `${id}-unit`, helper: `${id}-helper`, inherit: `${id}-inherit` }
  const clamp = clampTo(min, max)
  const inherited = props.nullable === true && props.value === null
  // The value "Inherit" off restores; captured in handlers, not during render.
  const [remembered, setRemembered] = useState(() => props.value ?? min)
  const shown = props.value ?? remembered

  // Both union arms accept a number; only the nullable arm accepts null.
  const emit = props.onChange as (value: number | null) => void
  const describedBy = [unit && ids.unit, helper && ids.helper].filter(Boolean).join(' ') || undefined

  function emitNumber(v: number) {
    setRemembered(v)
    emit(v)
  }

  function handleTyped(raw: string) {
    const parsed = raw === '' ? min : Number(raw)
    if (Number.isNaN(parsed)) return
    emitNumber(clamp(parsed))
  }

  function handleInherit(on: boolean) {
    if (!on) return emitNumber(remembered)
    setRemembered(shown)
    emit(null)
  }

  return (
    // A composite control: never inherit an outer Field's id/description.
    <FieldContext.Provider value={null}>
      <div className={cn('space-y-1.5', className)}>
        <div className="flex items-center justify-between gap-3">
          <Label id={ids.label} htmlFor={ids.input}>
            {label}
          </Label>
          {props.nullable && (
            <span className="flex items-center gap-2">
              <label htmlFor={ids.inherit} className="cursor-pointer text-caption text-text-secondary">
                {t('common.inherit')}
              </label>
              <Switch
                id={ids.inherit}
                checked={inherited}
                title={t('common.inherit.helper')}
                onCheckedChange={handleInherit}
              />
            </span>
          )}
        </div>
        <div className="flex items-center gap-3">
          <Slider
            min={min}
            max={max}
            step={step}
            value={clamp(shown)}
            disabled={inherited}
            onChange={emitNumber}
            aria-labelledby={ids.label}
          />
          <div className="flex items-center gap-1">
            <Input
              id={ids.input}
              type="number"
              inputMode="numeric"
              min={min}
              max={max}
              step={step}
              className="w-24"
              value={shown}
              disabled={inherited}
              aria-describedby={describedBy}
              onChange={(e) => handleTyped(e.target.value)}
            />
            {unit && (
              <span id={ids.unit} className="text-caption text-text-tertiary">
                {unit}
              </span>
            )}
          </div>
        </div>
        {helper && (
          <p id={ids.helper} className="text-body-sm text-text-tertiary">
            {helper}
          </p>
        )}
      </div>
    </FieldContext.Provider>
  )
}
