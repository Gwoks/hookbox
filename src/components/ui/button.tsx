/**
 * Button (design.md §3.0). Variants primary/secondary/ghost/danger/link.
 * primary = bg-accent-fill text-on-accent (teal); the single accent button per
 * surface (+ New Rule / Save). Loading preserves width + sets aria-busy. Mirrors
 * the reference button.tsx CVA pattern; only token classes differ.
 */
import { Slot } from '@radix-ui/react-slot'
import { cva, type VariantProps } from 'class-variance-authority'
import { forwardRef } from 'react'
import { cn } from '@/lib/cn'
import { Spinner } from './spinner'

const buttonVariants = cva(
  'inline-flex items-center justify-center gap-2 whitespace-nowrap rounded-sm font-medium transition-colors duration-fast ease-standard disabled:cursor-not-allowed disabled:bg-surface-subtle disabled:text-text-tertiary disabled:shadow-none',
  {
    variants: {
      variant: {
        primary: 'bg-accent-fill text-text-on-accent hover:opacity-90',
        secondary: 'border border-border-strong bg-surface text-text-primary hover:bg-surface-hover',
        ghost: 'text-text-secondary hover:bg-surface-hover hover:text-text-primary',
        danger: 'bg-danger-fg text-white hover:opacity-90',
        link: 'h-auto p-0 text-accent underline-offset-4 hover:underline',
      },
      size: {
        sm: 'h-7 px-3 text-body-sm',
        md: 'h-9 px-4 text-body-sm',
        lg: 'h-11 px-5 text-body',
        icon: 'h-9 w-9 p-0',
        'icon-sm': 'h-7 w-7 p-0',
      },
    },
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
