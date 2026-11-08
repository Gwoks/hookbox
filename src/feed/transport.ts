/**
 * Live-feed transport URLs — the ONE place they are built. No credential
 * ever appears in them: both the WebSocket upgrade and the EventSource
 * request are same-origin, so the browser attaches the owner's HttpOnly
 * session cookie itself, and the server checks the Origin header before
 * accepting it (hookbox-a73.3; the old `?cap=` query param is gone).
 */
type PageLocation = Pick<Location, 'protocol' | 'host'>

const feedPath = (kind: 'ws' | 'sse', token: string): string => `/${kind}/${encodeURIComponent(token)}`

/** WebSocket URL on the page's own host, ws: or wss: to match the page. */
export function feedSocketUrl(token: string, location: PageLocation = window.location): string {
  const scheme = location.protocol === 'https:' ? 'wss' : 'ws'
  return `${scheme}://${location.host}${feedPath('ws', token)}`
}

/** Same-origin Server-Sent Events URL (the WebSocket fallback). */
export function feedEventSourceUrl(token: string): string {
  return feedPath('sse', token)
}
