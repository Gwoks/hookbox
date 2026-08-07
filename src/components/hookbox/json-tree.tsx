/** JsonTree (design.md §3.6/§3.9, AC-44/AC-135/AC-136). radius-md container,
 * bg-surface-subtle, mono-sm. Keys text-secondary, strings success-fg-tinted,
 * numbers/booleans info-fg-tinted, null/punctuation text-tertiary (syntax tint
 * is decorative — structure + indent carry meaning). Pretty/Raw via Segmented,
 * defaulting to Raw for large or non-JSON bodies so the tree never tries to
 * walk something that isn't a parse tree. EVERY value is a plain text node
 * (XSS-inert, ux.md §6) — no dangerouslySetInnerHTML. */
import { useState } from 'react'
import { ChevronRight } from 'lucide-react'
import { cn } from '@/lib/cn'
import { t } from '@/lib/copy'
import { Segmented } from '@/components/ui/segmented'
import { CopyButton } from '@/components/ui/copy-button'

// Above this many bytes, default to Raw rather than walking a parse tree —
// a performance choice, not a failure (design.md §3.9, AC-135).
const LARGE_BODY_BYTES = 64 * 1024

// Mirrors the server's persisted-body cap (MAX_BODY_BYTES, backend/src/config.rs,
// default 256_000 — architecture.md §2.9). The API exposes no field carrying the
// configured value (F7 adds no new endpoint/column for it), so this heuristic
// can drift if an operator overrides the env var; that imprecision is inherent
// — AC-70's own note is why insp.body.truncated hedges with "may be cut short"
// rather than asserting truncation as fact.
const DEFAULT_MAX_BODY_BYTES = 256_000

function utf8ByteLength(s: string): number {
  return new TextEncoder().encode(s).length
}

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
  const text = raw ?? ''
  const bytes = utf8ByteLength(text)
  const isLarge = bytes > LARGE_BODY_BYTES
  const isAtCap = bytes === DEFAULT_MAX_BODY_BYTES

  let parsed: unknown
  let ok = false
  try {
    parsed = JSON.parse(text)
    ok = true
  } catch {
    ok = false
  }

  const [mode, setMode] = useState<'pretty' | 'raw'>(() => (isLarge || !ok ? 'raw' : 'pretty'))
  const renderingRaw = mode === 'raw' || !ok

  return (
    <div className={cn('rounded-md border border-border bg-surface-subtle', className)}>
      <div className="flex items-center justify-between border-b border-border px-2 py-1.5">
        <Segmented
          aria-label={t('insp.body.viewMode.aria')}
          options={[
            { value: 'pretty', label: t('insp.body.pretty') },
            { value: 'raw', label: t('insp.body.raw') },
          ]}
          value={mode}
          onChange={(v) => setMode(v)}
        />
        <CopyButton value={text} />
      </div>
      {renderingRaw && isLarge && (
        <p className="px-3 pb-2 pt-1.5 text-caption text-text-tertiary">{t('insp.body.largeRaw')}</p>
      )}
      {renderingRaw && !ok && (
        <p className="px-3 pb-2 pt-1.5 text-caption text-text-tertiary">{t('insp.body.notJson')}</p>
      )}
      {isAtCap && (
        <p className="px-3 pb-2 pt-1.5 text-caption text-text-tertiary">
          {t('insp.body.truncated', { bytes: DEFAULT_MAX_BODY_BYTES })}
        </p>
      )}
      <div className="max-h-[40vh] overflow-auto p-3 font-mono text-mono-sm">
        {mode === 'pretty' && ok ? (
          <Node value={parsed} depth={0} />
        ) : (
          <pre className="whitespace-pre-wrap break-all text-text-primary">
            {text || t('insp.headers.none')}
          </pre>
        )}
      </div>
    </div>
  )
}
