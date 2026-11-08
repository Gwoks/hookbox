/**
 * Button (DESIGN.md "Components"). Variants:
 *  - primary      the ONE filled blue action per screen (bg-accent-fill)
 *  - tint         DESIGN's "Ghost CTA": sky-tint fill, AA-strong blue text
 *  - ghost        no fill, no border — tertiary actions
 *  - outline      compact 4px-radius bordered action
 *  - danger       SOFT red (never a filled red button)
 *  - danger-ghost red text only
 *  - nav          muted nav link; `aria-current="page"` marks the active one
 *  - link         inline text link, no box
 * Loading preserves width, disables and sets aria-busy.
 */
import { Slot } from '@radix-ui/react-slot'
import { cva, type VariantProps } from 'class-variance-authority'
import { forwardRef } from 'react'
import { cn } from '@/lib/cn'
import { Spinner } from './spinner'

const OUTLINE =
  'rounded-xs border border-border-strong bg-transparent text-text-primary hover:bg-surface-hover'

const buttonVariants = cva(
  'inline-flex items-center justify-center gap-2 whitespace-nowrap rounded-sm font-medium transition-colors duration-base ease-standard disabled:cursor-not-allowed disabled:bg-surface-subtle disabled:text-text-tertiary disabled:shadow-none',
  {
    variants: {
      variant: {
        primary: 'bg-accent-fill text-text-on-accent hover:bg-accent-hover',
        tint: 'bg-accent-subtle-bg text-accent-strong hover:bg-accent-subtle-hover',
        ghost: 'text-text-secondary hover:bg-surface-hover hover:text-text-primary',
        outline: OUTLINE,
        /** @deprecated use `outline` */
        danger: 'border border-danger-border bg-danger-bg text-danger-fg hover:border-danger-fg',
        'danger-ghost': 'text-danger-fg hover:bg-danger-bg',
        nav: 'text-text-secondary hover:text-text-primary aria-[current=page]:bg-surface-hover aria-[current=page]:text-text-primary',
        link: 'text-accent-strong underline-offset-4 hover:underline',
      },
      size: {
        sm: 'h-7 px-3 text-body-sm',
        md: 'h-9 px-4 text-body-sm',
        lg: 'h-11 px-5 text-body',
        icon: 'h-9 w-9 p-0',
        'icon-sm': 'h-7 w-7 p-0',
      },
    },
    compoundVariants: [
      // DESIGN.md "Outlined Text Button": compact 5px/10px padding.
      { variant: 'outline', size: 'md', class: 'h-8 px-2.5' },
      // A link is text, not a box: no fixed height or padding at any size.
      { variant: 'link', class: 'h-auto px-0 py-0' },
    ],
    defaultVariants: { variant: 'primary', size: 'md' },
  },
)

export interface ButtonProps
  extends React.ButtonHTMLAttributes<HTMLButtonElement>,
    VariantProps<typeof buttonVariants> {
  loading?: boolean
  asChild?: boolean
}

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(function Button(
  { className, variant, size, loading, asChild, children, disabled, ...props },
  ref,
) {
  const Comp = asChild ? Slot : 'button'
  return (
    <Comp
      ref={ref}
      className={cn(buttonVariants({ variant, size }), className)}
      aria-busy={loading || undefined}
      disabled={asChild ? undefined : disabled || loading}
      {...props}
    >
      {asChild ? (
        children
      ) : (
        <>
          {loading && <Spinner className="h-4 w-4" />}
          {children}
        </>
      )}
    </Comp>
  )
})

export { buttonVariants }
