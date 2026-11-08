/**
 * useCsvExport — F5 "Export CSV" (AC-46..56, AC-115..121) as one state
 * machine: idle → fetching (n of total) → serialising → idle.
 *
 * - The caller passes the rows visible at activation (AC-115/116); buffered
 *   arrivals never join a run.
 * - Cancel aborts every in-flight detail fetch; "Preparing file…" cannot be
 *   cancelled (it is synchronous), so the UI disables Cancel there (AC-117).
 * - Rows whose detail could not be fetched leave a persistent, dismissible
 *   note (AC-121) on top of the toast.
 * - Unmounting aborts a running export (AC-53).
 */
import { useCallback, useEffect, useRef, useState } from 'react'
import type { RequestSummary } from '@/api/schemas'
import { t } from '@/lib/copy'
import { downloadBlob } from '@/lib/download'
import { buildRequestCsv, exportFilename, fetchDetails, isSentinelCell, type DetailCell } from '@/lib/request-export'
import { useToast } from '@/components/ui/toast'

export type ExportPhase =
  | { kind: 'idle' }
  | { kind: 'fetching'; done: number; total: number }
  | { kind: 'serialising'; total: number }

export interface CsvExport {
  phase: ExportPhase
  isExporting: boolean
  hasDetailNote: boolean
  dismissDetailNote: () => void
  start: (rows: readonly RequestSummary[]) => Promise<void>
  cancel: () => void
}

const IDLE: ExportPhase = { kind: 'idle' }
const CSV_MIME = 'text/csv;charset=utf-8'

export function useCsvExport(token: string): CsvExport {
  const { toast } = useToast()
  const [phase, setPhase] = useState<ExportPhase>(IDLE)
  const [hasDetailNote, setHasDetailNote] = useState(false)
  const controllerRef = useRef<AbortController | null>(null)

  useEffect(() => () => controllerRef.current?.abort(), [])

  const finish = useCallback(
    (snapshot: readonly RequestSummary[], details: ReadonlyArray<DetailCell>) => {
      const total = snapshot.length
      setPhase({ kind: 'serialising', total })
      try {
        downloadBlob(exportFilename(token, new Date()), CSV_MIME, buildRequestCsv(snapshot, details))
      } catch {
        setPhase(IDLE)
        toast(t('feed.export.error.file'), 'danger')
        return
      }
      setPhase(IDLE)
      const missing = details.filter(isSentinelCell).length
      setHasDetailNote(missing > 0)
      toast(missing > 0 ? t('feed.export.done.partial', { n: total, m: missing }) : t('feed.export.done', { n: total }))
    },
    [toast, token],
  )

  const start = useCallback(
    async (rows: readonly RequestSummary[]) => {
      const snapshot = rows.slice()
      const total = snapshot.length
      const controller = new AbortController()
      controllerRef.current = controller
      setHasDetailNote(false)
      setPhase({ kind: 'fetching', done: 0, total })

      let details: ReadonlyArray<DetailCell>
      try {
        details = await fetchDetails(snapshot, controller.signal, (done) => {
          if (!controller.signal.aborted) setPhase({ kind: 'fetching', done, total })
        })
      } catch {
        if (controllerRef.current === controller) {
          setPhase(IDLE)
          toast(t('feed.export.error'), 'danger')
        }
        return
      }
      if (controller.signal.aborted) {
        setPhase(IDLE)
        toast(t('feed.export.cancelled'))
        return
      }
      finish(snapshot, details)
    },
    [finish, toast],
  )

  const cancel = useCallback(() => controllerRef.current?.abort(), [])
  const dismissDetailNote = useCallback(() => setHasDetailNote(false), [])

  return { phase, isExporting: phase.kind !== 'idle', hasDetailNote, dismissDetailNote, start, cancel }
}
