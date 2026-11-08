import { act, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { ApiError } from '@/api/errors'
import type { EndpointDetail, EndpointSummary } from '@/api/schemas'
import { EndpointProvider, useEndpointState, useEndpoints, type EndpointContextValue } from './endpoint-context'

const api = vi.hoisted(() => ({ getEndpoint: vi.fn(), listEndpoints: vi.fn() }))
vi.mock('@/api', () => ({ api }))

const DETAIL: EndpointDetail = {
  token: 'toka1111',
  name: 'Checkout API',
  mock_url: 'https://toka1111.hookbox.test',
  path_url: 'http://localhost/e/toka1111',
  auto_crud: true,
  target_url: null,
  default_mode: 'mock_404',
  latency_ms: 0,
  rate_limit_per_min: 0,
  chaos_pct: 0,
  chaos_mode: 'error',
  cors_enabled: false,
  tunnel_active: false,
  created_at: '2026-11-08T00:00:00Z',
  last_hit: null,
  request_count: 3,
}

const SUMMARY: EndpointSummary = {
  token: DETAIL.token,
  name: DETAIL.name,
  mock_url: DETAIL.mock_url,
  path_url: DETAIL.path_url,
  created_at: DETAIL.created_at,
  last_hit: null,
  request_count: 3,
}

let latest: EndpointContextValue | null = null

function StateProbe() {
  const state = useEndpointState()
  if (state.status === 'loading') return <p>loading</p>
  if (state.status === 'error') {
    return (
      <button type="button" onClick={state.retry}>
        retry {String((state.error as ApiError).status)}
      </button>
    )
  }
  return <ReadyProbe />
}

function ReadyProbe() {
  const ctx = useEndpoints()
  latest = ctx
  return (
    <p>
      {ctx.endpoint.name} · rev {ctx.revision} · {ctx.endpoints.map((e) => e.name).join(',')}
    </p>
  )
}

function renderProvider() {
  return render(
    <EndpointProvider token={DETAIL.token}>
      <StateProbe />
    </EndpointProvider>,
  )
}

beforeEach(() => {
  latest = null
  api.getEndpoint.mockReset()
  api.listEndpoints.mockReset()
  api.getEndpoint.mockResolvedValue(DETAIL)
  api.listEndpoints.mockResolvedValue([SUMMARY])
})

describe('EndpointProvider', () => {
  it('loads the endpoint and the switcher list for the token', async () => {
    renderProvider()
    expect(screen.getByText('loading')).toBeInTheDocument()
    expect(await screen.findByText('Checkout API · rev 0 · Checkout API')).toBeInTheDocument()
    expect(api.getEndpoint).toHaveBeenCalledWith('toka1111', expect.objectContaining({ signal: expect.any(AbortSignal) }))
    expect(latest?.token).toBe('toka1111')
  })

  it('stays usable when only the sibling list fails', async () => {
    api.listEndpoints.mockRejectedValue(new ApiError('server_error', 'Boom.', 500))
    renderProvider()
    expect(await screen.findByText('Checkout API · rev 0 ·')).toBeInTheDocument()
    expect(latest?.endpoints).toEqual([])
  })

  it('exposes a load failure with a retry that fetches again', async () => {
    api.getEndpoint.mockRejectedValueOnce(new ApiError('server_error', 'Boom.', 500))
    renderProvider()
    await userEvent.click(await screen.findByRole('button', { name: 'retry 500' }))
    expect(await screen.findByText('Checkout API · rev 0 · Checkout API')).toBeInTheDocument()
    expect(api.getEndpoint).toHaveBeenCalledTimes(2)
  })

  it('replace() swaps the endpoint, renames its switcher entry and bumps the revision', async () => {
    renderProvider()
    await screen.findByText('Checkout API · rev 0 · Checkout API')
    act(() => latest?.replace({ ...DETAIL, name: 'Orders API' }))
    expect(screen.getByText('Orders API · rev 1 · Orders API')).toBeInTheDocument()
  })

  it('refresh() re-fetches silently without leaving the ready state', async () => {
    renderProvider()
    await screen.findByText('Checkout API · rev 0 · Checkout API')
    api.getEndpoint.mockResolvedValue({ ...DETAIL, tunnel_active: true })
    await act(async () => {
      await latest?.refresh()
    })
    expect(screen.queryByText('loading')).toBeNull()
    await waitFor(() => expect(latest?.endpoint.tunnel_active).toBe(true))
    expect(latest?.revision).toBe(0)
  })
})

describe('useEndpoints', () => {
  it('throws outside a ready EndpointProvider', () => {
    vi.spyOn(console, 'error').mockImplementation(() => {})
    expect(() => render(<ReadyProbe />)).toThrow(/EndpointProvider/)
  })
})
