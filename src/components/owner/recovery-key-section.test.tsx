import { render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter, Route, Routes, useParams } from 'react-router-dom'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { session } from '@/api/session'
import { RecoveryKeySection } from './recovery-key-section'

const fetchMock = vi.fn()

const ENDPOINT_B = {
  token: 'tokb2222',
  name: null,
  mock_url: 'https://tokb2222.hookbox.test',
  path_url: 'http://localhost/e/tokb2222',
  created_at: '2026-11-08T00:00:00Z',
  last_hit: null,
  request_count: 0,
}

function jsonResponse(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), { status, headers: { 'Content-Type': 'application/json' } })
}

type Handler = (url: string, init: RequestInit) => Response

function serve(handler: Handler): void {
  fetchMock.mockImplementation(async (url: string, init: RequestInit) => handler(url, init))
}

function routeCalls(): string[] {
  return fetchMock.mock.calls.map(([url, init]) => `${init?.method ?? 'GET'} ${url}`)
}

function DashboardStub() {
  const { token } = useParams()
  return <p>dashboard {token}</p>
}

function renderSection() {
  return render(
    <MemoryRouter initialEntries={['/d/toka1111/settings']}>
      <Routes>
        <Route path="/d/:token/settings" element={<RecoveryKeySection />} />
        <Route path="/d/:token" element={<DashboardStub />} />
      </Routes>
    </MemoryRouter>,
  )
}

beforeEach(() => {
  fetchMock.mockReset()
  vi.stubGlobal('fetch', fetchMock)
  session.markSignedIn('owner_a')
})
afterEach(() => vi.unstubAllGlobals())

describe('RecoveryKeySection', () => {
  it('is an anchored section that keeps the key hidden until asked', () => {
    renderSection()
    const section = screen.getByRole('region', { name: 'Recovery key' })
    expect(section).toHaveAttribute('id', 'recovery-key')
    expect(within(section).getByText('Recovery key hidden')).toBeInTheDocument()
    expect(fetchMock).not.toHaveBeenCalled()
  })

  it('reveals the key from POST /api/session/key and hides it again', async () => {
    const user = userEvent.setup()
    serve(() => jsonResponse({ key: 'key_alpha_0123456789' }))
    renderSection()

    await user.click(screen.getByRole('button', { name: 'Reveal key' }))

    expect(await screen.findByText('key_alpha_0123456789')).toBeInTheDocument()
    expect(routeCalls()).toEqual(['POST /api/session/key'])
    expect(screen.getByRole('button', { name: 'Copy key' })).toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: 'Hide key' }))
    expect(screen.queryByText('key_alpha_0123456789')).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'Copy key' })).not.toBeInTheDocument()
  })

  it('reports a failed reveal', async () => {
    const user = userEvent.setup()
    serve(() => jsonResponse({ error: 'server_error', detail: 'Boom.' }, 500))
    renderSection()

    await user.click(screen.getByRole('button', { name: 'Reveal key' }))

    expect(await screen.findByRole('alert')).toHaveTextContent('Boom.')
  })

  it('rotates only after confirming, then shows the new key', async () => {
    const user = userEvent.setup()
    serve(() => jsonResponse({ key: 'key_rotated_9876543210' }))
    renderSection()

    await user.click(screen.getByRole('button', { name: 'Rotate key' }))
    const dialog = await screen.findByRole('dialog')
    expect(within(dialog).getByText(/tunnel CLI disconnects/)).toBeInTheDocument()
    expect(fetchMock).not.toHaveBeenCalled()

    await user.click(within(dialog).getByRole('button', { name: 'Rotate key' }))

    expect(await screen.findByText('key_rotated_9876543210')).toBeInTheDocument()
    expect(screen.getByText(/New key ready/)).toBeInTheDocument()
    expect(routeCalls()).toEqual(['POST /api/session/rotate'])
    expect(session.getSnapshot()).toEqual({ status: 'signed-in', ownerId: 'owner_a' })
  })

  it('restores a different key and opens that owner’s primary endpoint', async () => {
    const user = userEvent.setup()
    serve(() => jsonResponse({ owner_id: 'owner_b', endpoints: [ENDPOINT_B], primary: ENDPOINT_B }))
    renderSection()

    await user.click(screen.getByRole('button', { name: 'Use a different recovery key' }))
    const input = screen.getByLabelText('Recovery key', { selector: 'input' })
    expect(input).toHaveAttribute('type', 'password')
    expect(input).toHaveAttribute('autocomplete', 'off')
    expect(input).toHaveAttribute('spellcheck', 'false')
    await user.type(input, '  key_bravo_0123456789  ')
    await user.click(screen.getByRole('button', { name: 'Restore access' }))

    expect(await screen.findByText('dashboard tokb2222')).toBeInTheDocument()
    expect(fetchMock.mock.calls[0][0]).toBe('/api/session/restore')
    expect(JSON.parse(fetchMock.mock.calls[0][1].body)).toEqual({ key: 'key_bravo_0123456789' })
    expect(session.getSnapshot()).toEqual({ status: 'signed-in', ownerId: 'owner_b' })
  })

  it('shows a field error and stays put when the key is rejected', async () => {
    const user = userEvent.setup()
    serve(() => jsonResponse({ error: 'invalid_key', detail: 'Unknown key.' }, 401))
    renderSection()

    await user.click(screen.getByRole('button', { name: 'Use a different recovery key' }))
    await user.type(screen.getByLabelText('Recovery key', { selector: 'input' }), 'nope')
    await user.click(screen.getByRole('button', { name: 'Restore access' }))

    expect(await screen.findByText(/That recovery key didn't work/)).toBeInTheDocument()
    expect(screen.getByLabelText('Recovery key', { selector: 'input' })).toHaveAttribute('aria-invalid', 'true')
    expect(session.getSnapshot().ownerId).toBe('owner_a')
  })

  it('asks for a key before calling the server', async () => {
    const user = userEvent.setup()
    renderSection()

    await user.click(screen.getByRole('button', { name: 'Use a different recovery key' }))
    await user.click(screen.getByRole('button', { name: 'Restore access' }))

    expect(screen.getByText('Paste your recovery key.')).toBeInTheDocument()
    await waitFor(() => expect(fetchMock).not.toHaveBeenCalled())
  })
})
