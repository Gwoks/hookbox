/** JsonTree (design.md §3.6, AC-44). radius-md container, bg-subtle, mono-sm.
 * Keys text-secondary, strings success-fg-tinted, numbers/booleans
 * info-fg-tinted, null/punctuation text-tertiary (syntax tint is decorative —
 * structure + indent carry meaning). Pretty/Raw via Segmented. EVERY value is a
 * plain text node (XSS-inert, ux.md §6) — no dangerouslySetInnerHTML. */
import { useState } from 'react'
import { ChevronRight } from 'lucide-react'
import { cn } from '@/lib/cn'
import { Segmented } from '@/components/ui/segmented'
import { CopyButton } from '@/components/ui/copy-button'

function Node({ value, k, depth }: { value: unknown; k?: string; depth: number }) {
  const [open, setOpen] = useState(depth < 2)
  const isObj = value !== null && typeof value === 'object'
  const keyLabel = k !== undefined ? <span className="text-text-secondary">{k}: </span> : null

  if (!isObj) {
    let cls = 'text-text-tertiary'
    if (typeof value === 'string') cls = 'text-success-fg'
    else if (typeof value === 'number' || typeof value === 'boolean') cls = 'text-info-fg'
    const text = typeof value === 'string' ? `"${value}"` : String(value)
    return (
      <div className="whitespace-pre-wrap break-all pl-[var(--indent)]" style={{ ['--indent' as string]: `${depth * 14}px` }}>
        {keyLabel}
        <span className={cls}>{text}</span>
      </div>
    )
  }

  const entries = Array.isArray(value)
    ? value.map((v, i) => [String(i), v] as const)
    : Object.entries(value as Record<string, unknown>)
  const open_b = Array.isArray(value) ? '[' : '{'
  const close_b = Array.isArray(value) ? ']' : '}'

  return (
    <div className="pl-[var(--indent)]" style={{ ['--indent' as string]: `${depth * 14}px` }}>
      <button
        type="button"
        onClick={() => setOpen((o) => !o)}
        className="inline-flex items-center gap-1 text-text-tertiary hover:text-text-secondary"
        aria-expanded={open}
      >
        <ChevronRight
          className={cn('h-3 w-3 transition-transform', open && 'rotate-90')}
          aria-hidden="true"
        />
        {keyLabel}
        <span className="text-text-tertiary">
          {open_b}
          {!open && `…${entries.length}${close_b}`}
        </span>
      </button>
      {open && (
        <div>
          {entries.map(([ck, cv]) => (
            <Node key={ck} k={ck} value={cv} depth={depth + 1} />
          ))}
          <div className="text-text-tertiary" style={{ paddingLeft: `${depth * 14}px` }}>
            {close_b}
          </div>
        </div>
      )}
    </div>
  )
}

export function JsonTree({ raw, className }: { raw: string | null | undefined; className?: string }) {
  const [mode, setMode] = useState<'pretty' | 'raw'>('pretty')
  const text = raw ?? ''
  let parsed: unknown
  let ok = false
  try {
    parsed = JSON.parse(text)
    ok = true
  } catch {
    ok = false
  }

  return (
    <div className={cn('rounded-md border border-border bg-subtle', className)}>
      <div className="flex items-center justify-between border-b border-border px-2 py-1.5">
        <Segmented
          aria-label="JSON view mode"
          options={[
            { value: 'pretty', label: 'Pretty' },
            { value: 'raw', label: 'Raw' },
          ]}
          value={mode}
          onChange={(v) => setMode(v)}
        />
        <CopyButton value={text} />
      </div>
      <div className="max-h-[40vh] overflow-auto p-3 font-mono text-mono-sm">
        {mode === 'pretty' && ok ? (
          <Node value={parsed} depth={0} />
        ) : (
          <pre className="whitespace-pre-wrap break-all text-text-primary">{text || '(empty)'}</pre>
        )}
      </div>
    </div>
  )
}
