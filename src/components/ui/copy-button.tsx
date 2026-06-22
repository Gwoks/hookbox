/** CopyButton (design.md §3.7/§6). Button → check + "Copied" for ~1.6s, plus an
 * sr-only aria-live announcement. Used by CodeBlock and KeyValueRows. */
import { Check, Copy } from 'lucide-react'
import { useCallback, useRef, useState } from 'react'
import { cn } from '@/lib/cn'
import { Button } from './button'

export function CopyButton({
  value,
  label = 'Copy',
  className,
  size = 'icon-sm',
}: {
  value: string
  label?: string
  className?: string
  size?: 'icon' | 'icon-sm' | 'sm'
}) {
  const [copied, setCopied] = useState(false)
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null)

  const onCopy = useCallback(async () => {
    try {
      await navigator.clipboard.writeText(value)
    } catch {
      /* clipboard unavailable */
    }
    setCopied(true)
    if (timer.current) clearTimeout(timer.current)
    timer.current = setTimeout(() => setCopied(false), 1600)
  }, [value])

  return (
    <Button
      variant="ghost"
      size={size}
      onClick={onCopy}
      aria-label={copied ? 'Copied' : label}
      className={cn('text-text-tertiary hover:text-text-primary', className)}
    >
      {copied ? (
        <Check className="h-4 w-4 text-success-fg" aria-hidden="true" />
      ) : (
        <Copy className="h-4 w-4" aria-hidden="true" />
      )}
      {size === 'sm' && <span>{copied ? 'Copied' : label}</span>}
      <span className="sr-only" role="status" aria-live="polite">
        {copied ? 'Copied to clipboard' : ''}
      </span>
    </Button>
  )
}
