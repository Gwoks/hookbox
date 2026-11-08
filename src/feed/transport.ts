/**
 * Live-feed transport URLs — the ONE place they are built. The connection
 * credential currently rides in `?cap=` (PRD §5.1 / AC-S14: never a cookie);
 * the identity stage moves it to the HttpOnly cookie + an Origin check, and
 * only this module changes when it does.
 */
type PageLocation = Pick<Location, 'protocol' | 'host'>

const feedPath = (kind: 'ws' | 'sse', token: string, cap: string): string =>
  `/${kind}/${encodeURIComponent(token)}?cap=${encodeURIComponent(cap)}`

/** WebSocket URL on the page's own host, ws: or wss: to match the page. */
export function feedSocketUrl(token: string, cap: string, location: PageLocation = window.location): string {
  const scheme = location.protocol === 'https:' ? 'wss' : 'ws'
  return `${scheme}://${location.host}${feedPath('ws', token, cap)}`
}

/** Same-origin Server-Sent Events URL (the WebSocket fallback). */
export function feedEventSourceUrl(token: string, cap: string): string {
  return feedPath('sse', token, cap)
}
