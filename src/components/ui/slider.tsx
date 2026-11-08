/** Slider (design.md §3.11). Track bg-surface-active radius-sm, filled
 * portion accent-fill. ALWAYS paired with a number input by the caller (see
 * NumberSlider). A native range input keeps it keyboard-accessible with no
 * extra dep; it picks up id / aria-describedby from a Field. */
import { cn } from '@/lib/cn'
import { useFieldControl } from './field'

export interface SliderProps {
  value: number
  min: number
  max: number
  step?: number
  onChange: (v: number) => void
  className?: string
  disabled?: boolean
  id?: string
  'aria-label'?: string
  'aria-labelledby'?: string
  'aria-describedby'?: string
}

export function Slider({
  value,
  min,
  max,
  step = 1,
  onChange,
  className,
  disabled,
  id,
  'aria-describedby': describedBy,
  ...aria
}: SliderProps) {
  const field = useFieldControl({ id, describedBy })
  const pct = max > min ? ((value - min) / (max - min)) * 100 : 0
  return (
    <input
      id={field.id}
      type="range"
      min={min}
      max={max}
      step={step}
      value={value}
      disabled={disabled}
      aria-describedby={field.describedBy}
      {...aria}
      onChange={(e) => onChange(Number(e.target.value))}
      className={cn(
        'h-1.5 w-full cursor-pointer appearance-none rounded-sm bg-surface-active disabled:cursor-not-allowed disabled:opacity-60',
        className,
      )}
      style={{
        // The filled track: the one flat two-stop fill DESIGN.md allows.
        background: `linear-gradient(to right, var(--accent-fill) ${pct}%, var(--bg-active) ${pct}%)`,
      }}
    />
  )
}
