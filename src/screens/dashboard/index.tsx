/**
 * Dashboard (/d/:token) — the operator's main screen (PRD §3, AC-44/45,
 * AC-D13/D18/D19, AC-J2/J3/J4/J10). The endpoint itself comes from
 * OwnerLayout's EndpointProvider (its loading / not-found / gone / error
 * frames live there); this screen owns the live feed and the inspector.
 *
 * - md+: a page-width SplitPane of two flush cards — feed left, inspector
 *   right. Below md the feed is full width and the selected row opens the
 *   inspector in a bottom Sheet.
 * - `endpoint_updated` (and Clear all, for request_count) silently
 *   refreshes the shared endpoint, which carries tunnel_active (AC-J10).
 * - Offline: a standing banner over the last-known data (AC-J13).
 */
import { useCallback, useEffect, useRef, useState } from 'react'
import { api } from '@/api'
import { connLabel, connTooltip, useFeed } from '@/feed'
import { remapEndpointGone } from '@/lib/api-errors'
import { t } from '@/lib/copy'
import { useMediaQuery } from '@/lib/hooks/use-media-query'
import { useOnline } from '@/lib/hooks/use-online'
import { ConfirmDialog } from '@/components/hookbox/confirm-dialog'
import { ConnectionPill } from '@/components/hookbox/connection-pill'
import { InlineAlert } from '@/components/hookbox/inline-alert'
import { SplitPane } from '@/components/hookbox/split-pane'
import { useEndpoints } from '@/components/owner/endpoint-context'
import { Card } from '@/components/ui/card'
import { useToast } from '@/components/ui/toast'
import { EndpointHeader } from './endpoint-header'
import { FeedPane } from './feed-pane'
import { Inspector } from './inspector'
import { MobileInspectorSheet } from './mobile-inspector-sheet'
import { SaveKeyAlert } from './save-key-alert'
import { useCsvExport } from './use-csv-export'

/** Tailwind's `md` breakpoint: the split view needs at least this width. */
const SPLIT_VIEW_QUERY = '(min-width: 768px)'

export function Dashboard() {
  const { token, endpoint, refresh } = useEndpoints()
  const { toast } = useToast()
  const isOnline = useOnline()
  const isSplitView = useMediaQuery(SPLIT_VIEW_QUERY)
  const csv = useCsvExport(token)
  const [isPaused, setIsPaused] = useState(false)
  const [selectedId, setSelectedId] = useState<number | null>(null)
  const [confirmClearOpen, setConfirmClearOpen] = useState(false)
  const [isLoaded, setIsLoaded] = useState(false)
  // Ids first seen on the live stream: their detail may 404 → "pending".
  const liveIds = useRef<Set<number>>(new Set())
  const prevTopId = useRef<number | null>(null)

  const onEndpointUpdated = useCallback(() => void refresh(), [refresh])
  const { rows, connState, attempt, newCount, flushBuffered, clearRows } = useFeed({
    token,
    paused: isPaused,
    onEndpointUpdated,
  })

  // Sticky "first content arrived" flag: rows, or the connection settled.
  useEffect(() => {
    if (rows.length > 0 || connState === 'live' || connState === 'sse') setIsLoaded(true)
  }, [rows.length, connState])

  useEffect(() => {
    const top = rows[0]?.id ?? null
    if (top !== null && top !== prevTopId.current) {
      liveIds.current.add(top)
      prevTopId.current = top
    }
  }, [rows])

  function togglePause() {
    if (isPaused) flushBuffered()
    setIsPaused((p) => !p)
  }

  // AC-76: enabled iff there is anything to act on (including the paused
  // buffer); never two reasons stacked — offline, then busy, then empty.
  const hasClearable = rows.length > 0 || newCount > 0
  const actionsDisabled = !hasClearable || csv.isExporting || !isOnline
  const actionsHint = !isOnline
    ? t('feed.actions.offlineHint')
    : csv.isExporting
      ? t('feed.actions.busyHint')
      : !hasClearable
        ? t('feed.actions.emptyHint')
        : null

  async function handleClearAll() {
    try {
      await api.clearRequests(token)
    } catch (err) {
      toast(t('feed.clearAll.error'), 'danger')
      remapEndpointGone(err)
    }
    clearRows()
    // AC-78: the inspector must not sit on a row that no longer exists.
    setSelectedId(null)
    liveIds.current.clear()
    // AC-79: refresh request_count rather than leaving it stale.
    void refresh()
    toast(t('set.toast.historyCleared'))
  }

  const isLive = selectedId !== null && liveIds.current.has(selectedId)
  const feedPane = (
    <FeedPane
      rows={rows}
      mockUrl={endpoint.mock_url}
      isLoaded={isLoaded}
      selectedId={selectedId}
      onSelect={setSelectedId}
      csv={csv}
      isPaused={isPaused}
      newCount={newCount}
      onFlush={flushBuffered}
      onTogglePause={togglePause}
      actionsDisabled={actionsDisabled}
      actionsHint={actionsHint}
      onClearAll={() => setConfirmClearOpen(true)}
    />
  )

  return (
    <div className="mx-auto flex h-[calc(100dvh-theme(spacing.nav))] min-h-[36rem] w-full max-w-page flex-col gap-4 px-4 py-5 sm:px-6">
      <EndpointHeader
        endpoint={endpoint}
        connection={<ConnectionPill state={connState} label={connLabel(connState, attempt)} title={connTooltip(connState)} />}
      />
      <SaveKeyAlert token={token} />
      {!isOnline && (
        <InlineAlert variant="warning" role="status" title={t('dash.state.offline.title')}>
          {t('dash.state.offline.body')}
        </InlineAlert>
      )}
      <div className="min-h-0 flex-1">
        {isSplitView ? (
          <SplitPane
            left={
              <Card padding="none" className="h-full overflow-hidden">
                {feedPane}
              </Card>
            }
            right={
              <Card padding="none" className="h-full overflow-hidden">
                <Inspector id={selectedId} isLive={isLive} />
              </Card>
            }
          />
        ) : (
          <Card padding="none" className="h-full overflow-hidden">
            {feedPane}
          </Card>
        )}
      </div>
      {!isSplitView && <MobileInspectorSheet id={selectedId} isLive={isLive} onClose={() => setSelectedId(null)} />}
      <ConfirmDialog
        open={confirmClearOpen}
        onClose={() => setConfirmClearOpen(false)}
        title={t('feed.clearAll.confirm.title')}
        body={
          <>
            <p>{t('feed.clearAll.confirm.body', { endpoint: endpoint.name || endpoint.token })}</p>
            <p className="mt-2 text-caption text-text-tertiary">{t('feed.clearAll.confirm.note')}</p>
          </>
        }
        confirmLabel={t('feed.clearAll.confirm.confirm')}
        errorFallback={t('feed.clearAll.error')}
        onConfirm={handleClearAll}
      />
    </div>
  )
}
