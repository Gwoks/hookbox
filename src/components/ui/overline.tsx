/** Overline — small uppercase label (section eyebrows, column headers,
 * inspector sub-headings). The `overline` type token already carries its
 * +0.06em tracking, so callers must not add `tracking-*`. */
import type { HTMLAttributes } from 'react'
import { cn } from '@/lib/cn'

type OverlineElement = 'p' | 'span' | 'div' | 'h2' | 'h3' | 'h4' | 'dt'

export interface OverlineProps extends HTMLAttributes<HTMLElement> {
  as?: OverlineElement
}

export function Overline({ as: Comp = 'p', className, ...props }: OverlineProps) {
  return <Comp className={cn('text-overline uppercase text-text-tertiary', className)} {...props} />
}
