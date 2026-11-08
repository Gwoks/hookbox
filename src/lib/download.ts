/** Shared client-side file download (design.md §9.1 notes, AC-12/AC-49).
 * Object-URL → programmatic anchor click → revoke in a `finally`, so F3's
 * config export and F5's CSV export can't drift into two implementations of
 * the same three steps. */

/** How long the object URL outlives the click. Chromium copes with an
 * immediate revoke, but Firefox/Safari can start the download after the
 * click returns and would then fetch a dead URL. */
const REVOKE_DELAY_MS = 30_000

/** Trigger a browser download of `bytes` as `filename`, with no server
 * round-trip. Revokes the object URL (deferred) even if the click throws. */
export function downloadBlob(filename: string, mime: string, bytes: BlobPart): void {
  const blob = new Blob([bytes], { type: mime })
  const url = URL.createObjectURL(blob)
  try {
    const a = document.createElement('a')
    a.href = url
    a.download = filename
    a.rel = 'noopener'
    document.body.appendChild(a)
    a.click()
    a.remove()
  } finally {
    setTimeout(() => URL.revokeObjectURL(url), REVOKE_DELAY_MS)
  }
}
