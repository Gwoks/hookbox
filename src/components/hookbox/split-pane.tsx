/** SplitPane (design.md §3.9 / §7). LEFT feed-pct / min feed-min-w, RIGHT
 * flex-1. Divider is a splitter-w hit area with a centered 1px border-strong
 * hairline; on hover/focus it thickens to 2px accent. Below bp-md it stacks
 * (the parent screen swaps to the mobile slide-over). Drag-to-resize keeps the
 * left width within [min, 70%]. */
import { useCallback, useRef, useState, type ReactNode } from 'react'
import { cn } from '@/lib/cn'

export function SplitPane({
  left,
  right,
  className,
  initialLeftPct = 40,
  minLeftPx = 360,
}: {
  left: ReactNode
  right: ReactNode
  className?: string
  initialLeftPct?: number
  minLeftPx?: number
}) {
  const [leftPct, setLeftPct] = useState(initialLeftPct)
  const ref = useRef<HTMLDivElement>(null)
  const dragging = useRef(false)

  const onMove = useCallback(
    (clientX: number) => {
      const el = ref.current
      if (!el) return
      const rect = el.getBoundingClientRect()
      const px = clientX - rect.left
      const minPct = (minLeftPx / rect.width) * 100
      const pct = Math.min(70, Math.max(minPct, (px / rect.width) * 100))
      setLeftPct(pct)
    },
    [minLeftPx],
  )

  const start = () => {
    dragging.current = true
    const onPointerMove = (e: PointerEvent) => dragging.current && onMove(e.clientX)
    const onPointerUp = () => {
      dragging.current = false
      window.removeEventListener('pointermove', onPointerMove)
      window.removeEventListener('pointerup', onPointerUp)
    }
    window.addEventListener('pointermove', onPointerMove)
    window.addEventListener('pointerup', onPointerUp)
  }

  return (
    <div ref={ref} className={cn('flex h-full min-h-0 w-full', className)}>
      <div className="min-w-feed overflow-hidden" style={{ flexBasis: `${leftPct}%` }}>
        {left}
      </div>
      <div
        role="separator"
        aria-orientation="vertical"
        tabIndex={0}
        onPointerDown={start}
        onKeyDown={(e) => {
          if (e.key === 'ArrowLeft') setLeftPct((p) => Math.max(20, p - 2))
          if (e.key === 'ArrowRight') setLeftPct((p) => Math.min(70, p + 2))
        }}
        className="group relative w-1.5 shrink-0 cursor-col-resize"
        aria-label="Resize feed and inspector"
      >
        <span className="absolute inset-y-0 left-1/2 w-px -translate-x-1/2 bg-border-strong transition-all group-hover:w-0.5 group-hover:bg-accent group-focus-visible:w-0.5 group-focus-visible:bg-accent" />
      </div>
      <div className="min-w-0 flex-1 overflow-hidden">{right}</div>
    </div>
  )
}
