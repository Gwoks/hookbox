/**
 * absolutize — turn a server-relative URL (`/e/<token>`) into a full
 * copy-pasteable URL using the page origin. The backend hands out relative
 * `mock_url`/`path_url` values in path-fallback mode when `PUBLIC_BASE_URL`
 * is not configured; the SPA always knows its own origin, so displayed and
 * copied URLs stay absolute either way. Already-absolute URLs pass through.
 */
export function absolutize(url: string): string {
  if (!url.startsWith('/') || typeof window === 'undefined') return url
  return window.location.origin + url
}
