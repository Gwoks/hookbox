/** Shared client-side file download (design.md §9.1 notes, AC-12/AC-49).
 * Object-URL → programmatic anchor click → revoke in a `finally`, so F3's
 * config export and F5's CSV export can't drift into two implementations of
 * the same three steps. */

/** Trigger a browser download of `bytes` as `filename`, with no server
 * round-trip. Revokes the object URL even if the click handler throws. */
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
    URL.revokeObjectURL(url)
  }
}
