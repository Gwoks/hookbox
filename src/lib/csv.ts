/** RFC 4180 CSV serializer for F5's frozen §5.6 artifact (operator-toolkit
 * prd.md §5.6, architecture D12). Pure and DOM-free — no fetch, no Blob, unit
 * testable without a browser. The three literals below go INTO the file, not
 * onto the screen, so they are constants here rather than copy.ts keys
 * (AC-64's deliberate exception): a copy pass must never reword, capitalise
 * or translate a machine-readable sentinel that downstream tools parse. */

export const CSV_HEADER = [
  'timestamp',
  'method',
  'path',
  'status_code',
  'served_by',
  'duration_ms',
  'request_headers',
  'request_body',
  'response_headers',
  'response_body',
] as const

/** The detail fetch 404'd — the documented just-streamed-trace case, not an
 * error (src/screens/dashboard/inspector.tsx's pending state). */
export const CSV_PENDING = 'pending'
/** Any other per-row failure (5xx, network, contract_mismatch, timeout). */
export const CSV_UNAVAILABLE = 'unavailable'

const GUARD_CHARS = new Set(['=', '+', '-', '@', '\t', '\r'])
const NEEDS_QUOTING = /["\r\n,]/

/** Guard-then-quote, in that frozen order (architecture D12). A leading
 * formula-trigger character is defused with a `'` prefix FIRST; whether to
 * wrap in quotes is decided on the (possibly guarded) value SECOND — so a
 * guarded value with no comma/quote/CR/LF stays unquoted. Applied uniformly
 * to all ten columns; it can never fire on the two bare-integer columns. */
export function escapeCell(value: string): string {
  let v = value
  if (v.length > 0 && GUARD_CHARS.has(v[0])) {
    v = `'${v}`
  }
  if (NEEDS_QUOTING.test(v)) {
    v = `"${v.replaceAll('"', '""')}"`
  }
  return v
}

/** Header row + one row per record, CRLF-separated, WITH a trailing CRLF
 * after the final record (frozen so fixtures are byte-stable). No BOM —
 * callers must not prepend one. */
export function toCsv(
  header: readonly string[],
  rows: readonly (readonly string[])[],
): string {
  const lines = [header, ...rows].map((cols) => cols.map(escapeCell).join(','))
  return lines.join('\r\n') + '\r\n'
}
