/** Map a feed ConnState (+ attempt count, +max conn) to the copy.md feed.conn.*
 * label + tooltip the ConnectionPill renders. The TEXT label carries the state
 * (AC-D17). Reconnecting shows the attempt count via feed.conn.reconnecting.n. */
import type { ConnState } from '@/components/hookbox/connection-pill'
import { t } from '@/lib/copy'

export function connLabel(state: ConnState, attempt: number): string {
  switch (state) {
    case 'connecting':
      return t('feed.conn.connecting')
    case 'live':
      return t('feed.conn.live')
    case 'reconnecting':
    case 'degraded':
      return attempt > 0 ? t('feed.conn.reconnecting.n', { n: attempt }) : t('feed.conn.reconnecting')
    case 'sse':
      return t('feed.conn.sse')
    case 'offline':
      return t('feed.conn.offline')
    case 'unauthorized':
      return t('feed.conn.unauthorized')
    case 'busy':
      return t('feed.conn.busy')
  }
}

export function connTooltip(state: ConnState, max = 50): string | undefined {
  switch (state) {
    case 'sse':
      return t('feed.conn.sse.tooltip')
    case 'offline':
      return t('feed.conn.offline.tooltip')
    case 'unauthorized':
      return t('feed.conn.unauthorized.tooltip')
    case 'busy':
      return t('feed.conn.busy.tooltip', { max })
    default:
      return undefined
  }
}
