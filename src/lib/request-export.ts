/** F5 CSV export orchestration (operator-toolkit prd.md §4.5/§5.6). Fetches
 * each visible row's detail through a bounded worker pool, then hands the
 * results to src/lib/csv.ts for serialisation. Kept out of dashboard.tsx so
 * the screen doesn't grow (architecture.md §1). */
import { api, ApiError, type RequestDetail, type RequestSummary } from '@/api'
import { CSV_HEADER, CSV_PENDING, CSV_UNAVAILABLE, toCsv } from './csv'

/** Exactly 4 in flight (§5.6), pulling from a shared cursor rather than
 * chunking, so a slow row never idles the other three workers. */
export const EXPORT_CONCURRENCY = 4

/** AC-119: a per-row fetch cannot hang the strip forever. `GET /api/requests/
 * {id}` carries no rate limit today; if one is ever added, this is the one
 * place a 429 Retry-After would be honoured. */
const ROW_TIMEOUT_MS = 10_000

export interface DetailCell {
  request_headers: string
  request_body: string
  response_headers: string
  response_body: string
}

function cellsFor(detail: RequestDetail): DetailCell {
  return {
    request_headers: JSON.stringify(detail.request_headers),
    request_body: detail.request_body ?? '',
    response_headers: JSON.stringify(detail.response_headers),
    response_body: detail.response_body ?? '',
  }
}

function sentinelCells(sentinel: typeof CSV_PENDING | typeof CSV_UNAVAILABLE): DetailCell {
  return {
    request_headers: sentinel,
    request_body: sentinel,
    response_headers: sentinel,
    response_body: sentinel,
  }
}

function withTimeout<T>(promise: Promise<T>, ms: number): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error('request-export: row timed out')), ms)
    promise.then(
      (v) => {
        clearTimeout(timer)
        resolve(v)
      },
      (e) => {
        clearTimeout(timer)
        reject(e)
      },
    )
  })
}

async function fetchOne(id: number, signal: AbortSignal): Promise<DetailCell> {
  try {
    const detail = await withTimeout(api.getRequest(id, { signal }), ROW_TIMEOUT_MS)
    return cellsFor(detail)
  } catch (err) {
    // A 404 is the documented just-streamed-trace case (pending); every
    // other failure — 5xx, network, contract_mismatch, our own timeout, an
    // abort — is unavailable. Per-row failure never aborts the export
    // (AC-52); an abort's sentinel result is discarded by the caller anyway
    // (AC-48/AC-53), since the worker loop returns without reporting progress.
    const status = err instanceof ApiError ? err.status : 0
    return sentinelCells(status === 404 ? CSV_PENDING : CSV_UNAVAILABLE)
  }
}

/** Fetch every row's detail with a fixed pool of `EXPORT_CONCURRENCY`
 * workers pulling from a shared cursor, so completion order never affects
 * row order — results land in a pre-sized array BY INDEX. `onProgress` fires
 * once per settled row. Cancellation (AC-48/AC-53): once `signal` is
 * aborted, workers stop pulling new work AND every in-flight
 * `GET /api/requests/{id}` is aborted through this same shared
 * `AbortController` (threaded into `api.getRequest`) — nothing keeps running
 * in the background. */
export async function fetchDetails(
  rows: readonly RequestSummary[],
  signal: AbortSignal,
  onProgress: (done: number, total: number) => void,
): Promise<ReadonlyArray<DetailCell>> {
  const total = rows.length
  const results: DetailCell[] = new Array(total)
  let cursor = 0
  let done = 0

  async function worker(): Promise<void> {
    while (!signal.aborted) {
      const i = cursor++
      if (i >= total) return
      results[i] = await fetchOne(rows[i].id, signal)
      if (signal.aborted) return
      done += 1
      onProgress(done, total)
    }
  }

  const workerCount = Math.min(EXPORT_CONCURRENCY, total)
  await Promise.all(Array.from({ length: workerCount }, worker))
  return results
}

/** True when a cell carries a sentinel rather than real (possibly empty)
 * content — used to decide the completion toast's "{m} without detail" and
 * whether AC-121's persistent detail note should appear. */
export function isSentinelCell(cell: DetailCell): boolean {
  return cell.request_headers === CSV_PENDING || cell.request_headers === CSV_UNAVAILABLE
}

/** Build the frozen §5.6 CSV body. The six summary columns always come from
 * the feed row — NEVER the detail response — so they are independent of the
 * detail fetch outcome (AC-51/AC-52). */
export function buildRequestCsv(
  rows: readonly RequestSummary[],
  details: ReadonlyArray<DetailCell>,
): string {
  const dataRows = rows.map((row, i) => {
    const d = details[i]
    return [
      row.timestamp,
      row.method,
      row.path,
      String(row.status_code),
      row.served_by,
      String(row.duration_ms),
      d.request_headers,
      d.request_body,
      d.response_headers,
      d.response_body,
    ]
  })
  return toCsv(CSV_HEADER, dataRows)
}

/** `hookbox-requests-<token>-<YYYYMMDDTHHMMSSZ>.csv` — the stamp is
 * `now.toISOString()` with `-`, `:` and the `.mmm` fraction removed. */
export function exportFilename(token: string, now: Date): string {
  const stamp = now.toISOString().replace(/[-:]/g, '').replace(/\.\d{3}Z$/, 'Z')
  return `hookbox-requests-${token}-${stamp}.csv`
}
