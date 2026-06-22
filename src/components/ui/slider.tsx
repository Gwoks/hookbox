/** Slider (design.md §3.11). Track bg-active radius-sm, filled portion
 * accent-fill, thumb radius-full bg-surface + border-strong (focus ring on
 * thumb). ALWAYS paired with a number input by the caller (latency / chaos /
 * rate). A native range input keeps it keyboard-accessible with no extra dep. */
import { cn } from '@/lib/cn'

export interface SliderProps {
  value: number
  min: number
  max: number
  step?: number
  onChange: (v: number) => void
  className?: string
  'aria-label'?: string
  id?: string
}

export function Slider({ value, min, max, step = 1, onChange, className, id, ...rest }: SliderProps) {
  const pct = max > min ? ((value - min) / (max - min)) * 100 : 0
  return (
    <input
      id={id}
      type="range"
      min={min}
      max={max}
      step={step}
      value={value}
      aria-label={rest['aria-label']}
      onChange={(e) => onChange(Number(e.target.value))}
      className={cn('hb-slider h-1.5 w-full cursor-pointer appearance-none rounded-sm bg-active', className)}
      style={{
        background: `linear-gradient(to right, var(--accent-fill) ${pct}%, var(--bg-active) ${pct}%)`,
      }}
    />
  )
}
