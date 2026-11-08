import { render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import type { ReactNode } from 'react'
import { MemoryRouter, Route, Routes, useLocation } from 'react-router-dom'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { EndpointDetail } from '@/api/schemas'
import { EndpointProvider, useEndpointState } from './endpoint-context'
import { TopNav } from './top-nav'

const api = vi.hoisted(() => ({ getEndpoint: vi.fn(), listEndpoints: vi.fn(), createEndpoint: vi.fn() }))
vi.mock('@/api', () => ({ api, useSession: () => ({ ownerId: 'a1b2c3d4e5f60718' }) }))

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

function Ready({ children }: { children: ReactNode }) {
  return useEndpointState().status === 'ready' ? <>{children}</> : null
}

function Where() {
  const { pathname } = useLocation()
  return <p data-testid="where">{pathname}</p>
}

const onShare = vi.fn()
const onSignOut = vi.fn()

function renderNav(path: string, shareCount: number | null = null) {
  return render(
    <MemoryRouter initialEntries={[path]}>
      <Routes>
        <Route
          path="/d/:token/*"
          element={
            <EndpointProvider token="toka1111">
              <Ready>
                <TopNav shareCount={shareCount} onShare={onShare} onSignOut={onSignOut} />
                <Where />
              </Ready>
            </EndpointProvider>
          }
        />
      </Routes>
    </MemoryRouter>,
  )
}

/** jsdom applies no CSS, so both the desktop bar and the sheet trigger
 * render; scope desktop assertions to the banner's own nav. */
async function desktopNav() {
  const banner = await screen.findByRole('banner')
  return within(banner).getByRole('navigation', { name: 'Main' })
}

beforeEach(() => {
  vi.clearAllMocks()
  api.getEndpoint.mockResolvedValue(DETAIL)
  api.listEndpoints.mockResolvedValue([DETAIL])
})

describe('TopNav', () => {
  it('marks the current section with aria-current', async () => {
    renderNav('/d/toka1111/rules')
    const nav = await desktopNav()
    expect(within(nav).getByRole('link', { name: 'Rules' })).toHaveAttribute('aria-current', 'page')
    expect(within(nav).getByRole('link', { name: 'Requests' })).not.toHaveAttribute('aria-current')
    expect(within(nav).getByRole('link', { name: 'Settings' })).toHaveAttribute('href', '/d/toka1111/settings')
  })

  it('treats Requests as exact so it is not current on sub-pages', async () => {
    renderNav('/d/toka1111')
    const nav = await desktopNav()
    expect(within(nav).getByRole('link', { name: 'Requests' })).toHaveAttribute('aria-current', 'page')
    expect(within(nav).getByRole('link', { name: 'Rules' })).not.toHaveAttribute('aria-current')
  })

  it('offers the New rule shortcut everywhere except the rules page', async () => {
    const { unmount } = renderNav('/d/toka1111/settings')
    expect(await screen.findByRole('link', { name: 'New rule' })).toHaveAttribute(
      'href',
      '/d/toka1111/rules?new=1',
    )
    unmount()
    renderNav('/d/toka1111/rules')
    await desktopNav()
    expect(screen.queryByRole('link', { name: 'New rule' })).toBeNull()
  })

  it('names the Share action with the active-link count', async () => {
    renderNav('/d/toka1111', 2)
    const share = await screen.findByRole('button', { name: 'Share this endpoint read-only — 2 active links' })
    await userEvent.click(share)
    expect(onShare).toHaveBeenCalledTimes(1)
  })

  it('creates a new endpoint from the switcher and opens it', async () => {
    api.createEndpoint.mockResolvedValue({ ...DETAIL, token: 'tokb2222', name: null })
    const user = userEvent.setup()
    renderNav('/d/toka1111')
    await user.click(await screen.findByRole('button', { name: 'Switch endpoint' }))
    await user.click(await screen.findByRole('menuitem', { name: 'Create a new endpoint' }))
    expect(api.createEndpoint).toHaveBeenCalledWith({})
    await waitFor(() => expect(screen.getByTestId('where')).toHaveTextContent('/d/tokb2222'))
  })

  it('opens the mobile Menu sheet and returns focus to its trigger on Escape', async () => {
    const user = userEvent.setup()
    renderNav('/d/toka1111')
    const trigger = await screen.findByRole('button', { name: 'Open menu' })
    await user.click(trigger)
    const sheet = await screen.findByRole('dialog', { name: 'Menu' })
    expect(within(sheet).getByRole('link', { name: 'Rules' })).toBeInTheDocument()
    expect(within(sheet).getByRole('link', { name: 'New rule' })).toBeInTheDocument()
    expect(within(sheet).getByRole('button', { name: 'Sign out' })).toBeInTheDocument()
    await user.keyboard('{Escape}')
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Menu' })).toBeNull())
    expect(trigger).toHaveFocus()
  })

  it('runs Sign out from the sheet only after the sheet closes', async () => {
    const user = userEvent.setup()
    renderNav('/d/toka1111')
    await user.click(await screen.findByRole('button', { name: 'Open menu' }))
    const sheet = await screen.findByRole('dialog', { name: 'Menu' })
    await user.click(within(sheet).getByRole('button', { name: 'Sign out' }))
    await waitFor(() => expect(onSignOut).toHaveBeenCalledTimes(1))
    expect(screen.queryByRole('dialog', { name: 'Menu' })).toBeNull()
  })
})
