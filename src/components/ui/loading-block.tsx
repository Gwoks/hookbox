/** LoadingBlock — the shared "content is loading" placeholder: aria-busy,
 * an sr-only label for assistive tech, and decorative skeleton lines (or a
 * custom skeleton passed as children). Static under reduced motion. */
import type { ReactNode } from 'react'
import { cn } from '@/lib/cn'
import { SkeletonLines } from './skeleton'

export function LoadingBlock({
  label,
  lines = 3,
  className,
  children,
}: {
  label: string
  lines?: number
  className?: string
  children?: ReactNode
}) {
  return (
    <div aria-busy="true" className={cn(className)}>
      <span className="sr-only">{label}</span>
      {children ?? <SkeletonLines lines={lines} />}
    </div>
  )
}
