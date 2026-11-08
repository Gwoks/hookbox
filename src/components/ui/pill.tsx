/**
 * Pill (DESIGN.md "Pill Tag" / "Hero Highlight Pill"). 9999px radius only —
 * the one shape besides 12px cards. `hero` wraps a single word of a headline
 * (inheriting the headline's size) to give it weight; `none` is a bare
 * shape for callers that bring their own colors.
 */
import { cva, type VariantProps } from 'class-variance-authority'
import type { HTMLAttributes } from 'react'
import { cn } from '@/lib/cn'

export const pillVariants = cva('inline-flex items-center gap-1 whitespace-nowrap rounded-pill font-medium', {
  variants: {
    tone: {
      neutral: 'bg-neutral-chip-bg text-neutral-chip-fg',
      accent: 'bg-accent-subtle-bg text-accent-strong',
      success: 'bg-success-bg text-success-fg',
      warning: 'bg-warning-bg text-warning-fg',
      danger: 'bg-danger-bg text-danger-fg',
      info: 'bg-info-bg text-info-fg',
      marigold: 'bg-card-marigold text-card-marigold-fg',
      coral: 'bg-card-coral text-card-coral-fg',
      peach: 'bg-card-peach text-card-peach-fg',
      sky: 'bg-card-sky text-card-sky-fg',
      none: '',
    },
    size: {
      sm: 'px-2 py-0.5 text-caption',
      md: 'px-3 py-1 text-body-sm',
      hero: 'px-6 py-2',
    },
  },
  defaultVariants: { tone: 'neutral', size: 'sm' },
})

export interface PillProps extends HTMLAttributes<HTMLSpanElement>, VariantProps<typeof pillVariants> {}

export function Pill({ tone, size, className, ...props }: PillProps) {
  return <span className={cn(pillVariants({ tone, size }), className)} {...props} />
}
