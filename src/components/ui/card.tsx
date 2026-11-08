/**
 * Card (DESIGN.md "White / Accent / Dark Feature Card"). `surface` is the
 * default white card: 12px radius, hairline border, 24px padding, NO shadow.
 * Accent tones paint the card with a full hue (light) or a 16% wash (dark)
 * via the `card.<tone>` tokens; text inherits the tone's AA-checked fg.
 */
import { cva, type VariantProps } from 'class-variance-authority'
import { createContext, useContext, type HTMLAttributes, type ReactNode } from 'react'
import { cn } from '@/lib/cn'

export const cardVariants = cva('rounded-md border', {
  variants: {
    tone: {
      surface: 'border-border bg-surface text-text-primary',
      marigold: 'border-card-marigold-border bg-card-marigold text-card-marigold-fg',
      coral: 'border-card-coral-border bg-card-coral text-card-coral-fg',
      sky: 'border-card-sky-border bg-card-sky text-card-sky-fg',
      tint: 'border-card-tint-border bg-card-tint text-card-tint-fg',
      peach: 'border-card-peach-border bg-card-peach text-card-peach-fg',
      midnight: 'border-card-midnight-border bg-card-midnight text-card-midnight-fg',
    },
    padding: { none: '', sm: 'p-4', md: 'p-6', lg: 'p-8' },
  },
  defaultVariants: { tone: 'surface', padding: 'md' },
})

export type CardTone = NonNullable<VariantProps<typeof cardVariants>['tone']>
type CardElement = 'div' | 'section' | 'article' | 'aside' | 'li'

const CardToneContext = createContext<CardTone>('surface')

export interface CardProps extends HTMLAttributes<HTMLElement>, VariantProps<typeof cardVariants> {
  as?: CardElement
}

export function Card({ as: Comp = 'div', tone, padding, className, ...props }: CardProps) {
  return (
    <CardToneContext.Provider value={tone ?? 'surface'}>
      <Comp className={cn(cardVariants({ tone, padding }), className)} {...props} />
    </CardToneContext.Provider>
  )
}

export function CardHeader({
  title,
  description,
  actions,
  titleAs: Title = 'h2',
  className,
}: {
  title: ReactNode
  description?: ReactNode
  actions?: ReactNode
  titleAs?: 'h2' | 'h3' | 'h4'
  className?: string
}) {
  // Secondary text only on white cards; accent cards keep their own fg (AA).
  const isSurface = useContext(CardToneContext) === 'surface'
  return (
    <div className={cn('flex flex-wrap items-start justify-between gap-3', className)}>
      <div className="min-w-0 space-y-1">
        <Title className="text-h4">{title}</Title>
        {description && (
          <p className={cn('text-body-sm', isSurface && 'text-text-secondary')}>{description}</p>
        )}
      </div>
      {actions && <div className="flex shrink-0 flex-wrap items-center gap-2">{actions}</div>}
    </div>
  )
}
