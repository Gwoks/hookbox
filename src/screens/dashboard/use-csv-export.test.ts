import { act, renderHook } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { RequestSummary } from '@/api/schemas'
import { useCsvExport } from './use-csv-export'

const mocks = vi.hoisted(() => ({
  toast: vi.fn(),
  fetchDetails: vi.fn(),
  downloadBlob: vi.fn(),
}))

vi.mock('@/components/ui/toast', () => ({ useToast: () => ({ toast: mocks.toast }) }))
vi.mock('@/lib/download', () => ({ downloadBlob: mocks.downloadBlob }))
vi.mock('@/lib/request-export', () => ({
  fetchDetails: mocks.fetchDetails,
  buildRequestCsv: () => 'csv-body',
  exportFilename: (token: string) => `hookbox-${token}.csv`,
  isSentinelCell: (cell: { request_headers: string }) => cell.request_headers === 'pending',
}))

const ROW: RequestSummary = {
  id: 1,
  token: 'toka1111',
  method: 'GET',
  path: '/ping',
  status_code: 200,
  served_by: 'default',
  matched_rule_id: null,
  duration_ms: 4,
  overhead_ms: 1,
  timestamp: '2026-11-08T00:00:00Z',
}
const CELL = { request_headers: '{}', request_body: '', response_headers: '{}', response_body: '' }

beforeEach(() => vi.clearAllMocks())

describe('useCsvExport', () => {
  it('downloads the snapshot as CSV, reports progress and returns to idle', async () => {
    mocks.fetchDetails.mockImplementation(async (_rows, _signal, onProgress: (n: number) => void) => {
      onProgress(1)
      return [CELL]
    })
    const { result } = renderHook(() => useCsvExport('toka1111'))
    await act(() => result.current.start([ROW]))
    expect(mocks.downloadBlob).toHaveBeenCalledWith('hookbox-toka1111.csv', 'text/csv;charset=utf-8', 'csv-body')
    expect(mocks.toast).toHaveBeenCalledWith('Exported 1 requests.')
    expect(result.current.phase).toEqual({ kind: 'idle' })
    expect(result.current.isExporting).toBe(false)
    expect(result.current.hasDetailNote).toBe(false)
  })

  it('flags rows without detail with a persistent note', async () => {
    mocks.fetchDetails.mockResolvedValue([{ ...CELL, request_headers: 'pending' }])
    const { result } = renderHook(() => useCsvExport('toka1111'))
    await act(() => result.current.start([ROW]))
    expect(mocks.toast).toHaveBeenCalledWith('Exported 1 requests — 1 without detail.')
    expect(result.current.hasDetailNote).toBe(true)
    act(() => result.current.dismissDetailNote())
    expect(result.current.hasDetailNote).toBe(false)
  })

  it('cancels mid-fetch without downloading', async () => {
    let finish!: (cells: unknown[]) => void
    mocks.fetchDetails.mockImplementation(() => new Promise((resolve) => (finish = resolve)))
    const { result } = renderHook(() => useCsvExport('toka1111'))
    let run!: Promise<void>
    act(() => {
      run = result.current.start([ROW])
    })
    expect(result.current.phase).toEqual({ kind: 'fetching', done: 0, total: 1 })
    act(() => result.current.cancel())
    await act(async () => {
      finish([CELL])
      await run
    })
    expect(mocks.downloadBlob).not.toHaveBeenCalled()
    expect(mocks.toast).toHaveBeenCalledWith('Export cancelled. No file was downloaded.')
    expect(result.current.phase).toEqual({ kind: 'idle' })
  })

  it('reports a failed fetch as an error toast', async () => {
    mocks.fetchDetails.mockRejectedValue(new Error('network'))
    const { result } = renderHook(() => useCsvExport('toka1111'))
    await act(() => result.current.start([ROW]))
    expect(mocks.toast).toHaveBeenCalledWith("Couldn't finish the export. No file was downloaded.", 'danger')
    expect(mocks.downloadBlob).not.toHaveBeenCalled()
  })
})
