/** Shared relative-time formatting (design.md §2.2 `.tnum`/§10.1 notes).
 * Extracted from feed-row.tsx so the share dialog, the public viewer, the
 * export progress strip and any future "Updated {when}" caption format
 * timestamps identically instead of drifting. */

/** Format an RFC3339 timestamp as a short relative age ("12s", "4m", "3h",
 * "2d"). Returns '' for an unparseable input rather than throwing. */
export function relTime(iso: string): string {
  const t = Date.parse(iso)
  if (Number.isNaN(t)) return ''
  const diff = Math.max(0, Date.now() - t)
  const s = Math.floor(diff / 1000)
  if (s < 60) return `${s}s`
  const m = Math.floor(s / 60)
  if (m < 60) return `${m}m`
  const h = Math.floor(m / 60)
  if (h < 24) return `${h}h`
  return `${Math.floor(h / 24)}d`
}
