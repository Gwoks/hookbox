import { act, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter, Route, Routes } from 'react-router-dom'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { ApiError } from '@/api/errors'
import { t } from '@/lib/copy'
import { GetStarted } from './get-started'

interface TurnstileStubProps {
  siteKey: string
  options?: Record<string, unknown>
  onSuccess?: (token: string) => void
  onExpire?: () => void
  onError?: (code?: string) => void
}

const turnstile = vi.hoisted(() => ({
  props: null as TurnstileStubProps | null,
  reset: vi.fn(),
}))

vi.mock('@marsidev/react-turnstile', async () => {
  const React = await import('react')
  const Turnstile = React.forwardRef<{ reset: () => void }, TurnstileStubProps>((props, ref) => {
    turnstile.props = props
    React.useImperativeHandle(ref, () => ({ reset: turnstile.reset }))
    return <div data-testid="turnstile" />
  })
  return { Turnstile }
})

const config = vi.hoisted(() => ({ getTurnstileSiteKey: vi.fn<() => string | null>() }))
vi.mock('./turnstile-config', () => config)

const SUMMARY = { owner_id: 'owner_a', endpoints: [], primary: null }
const api = vi.hoisted(() => ({ createSession: vi.fn() }))
const session = vi.hoisted(() => ({ markSignedIn: vi.fn() }))
vi.mock('@/api', () => ({ api, session, resolvePrimaryToken: async () => 'tok12345' }))

const SITE_KEY = '1x00000000000000000000AA'
const START = t('landing.start.submit')
const CHECKING = t('landing.start.checking')
const HUMAN_ERROR = t('landing.error.turnstile')

function renderGetStarted() {
  return render(
    <MemoryRouter initialEntries={['/']}>
      <Routes>
        <Route path="/" element={<GetStarted hasResumeFailed={false} areCookiesEnabled />} />
        <Route path="/d/:token" element={<p>dashboard</p>} />
      </Routes>
    </MemoryRouter>,
  )
}

function widget(): TurnstileStubProps {
  if (!turnstile.props) throw new Error('Turnstile widget was not rendered')
  return turnstile.props
}

function solveChallenge(token: string) {
  act(() => widget().onSuccess?.(token))
}

beforeEach(() => {
  turnstile.props = null
  turnstile.reset.mockReset()
  api.createSession.mockReset()
  session.markSignedIn.mockReset()
  config.getTurnstileSiteKey.mockReset()
})

describe('GetStarted without a Turnstile site key', () => {
  beforeEach(() => config.getTurnstileSiteKey.mockReturnValue(null))

  it('renders no widget and mints without a token', async () => {
    api.createSession.mockResolvedValue(SUMMARY)
    renderGetStarted()

    expect(screen.queryByTestId('turnstile')).not.toBeInTheDocument()
    const button = screen.getByRole('button', { name: START })
    expect(button).toBeEnabled()

    await userEvent.click(button)

    expect(await screen.findByText('dashboard')).toBeInTheDocument()
    expect(api.createSession).toHaveBeenCalledTimes(1)
    expect(api.createSession.mock.calls[0][0]).toBeUndefined()
  })
})

describe('GetStarted with a Turnstile site key', () => {
  beforeEach(() => config.getTurnstileSiteKey.mockReturnValue(SITE_KEY))

  it('renders the session_mint widget in interaction-only mode', () => {
    renderGetStarted()

    expect(screen.getByTestId('turnstile')).toBeInTheDocument()
    expect(widget().siteKey).toBe(SITE_KEY)
    expect(widget().options).toMatchObject({
      action: 'session_mint',
      appearance: 'interaction-only',
      theme: 'auto',
    })
  })

  it('keeps the button disabled with the checking label until a token arrives', () => {
    renderGetStarted()

    expect(screen.getByRole('button', { name: CHECKING })).toBeDisabled()
    solveChallenge('cf-token-1')
    expect(screen.getByRole('button', { name: START })).toBeEnabled()
  })

  it('forwards the token, resets the widget and opens the dashboard on success', async () => {
    api.createSession.mockResolvedValue(SUMMARY)
    renderGetStarted()
    solveChallenge('cf-token-1')

    await userEvent.click(screen.getByRole('button', { name: START }))

    expect(await screen.findByText('dashboard')).toBeInTheDocument()
    expect(api.createSession).toHaveBeenCalledWith('cf-token-1')
    expect(turnstile.reset).toHaveBeenCalledTimes(1)
    expect(session.markSignedIn).toHaveBeenCalledWith('owner_a', { broadcast: true })
  })

  it('shows the friendly alert, resets the widget and clears the token on turnstile_failed', async () => {
    api.createSession.mockRejectedValue(new ApiError('turnstile_failed', 'Turnstile verification failed.', 403))
    renderGetStarted()
    solveChallenge('cf-token-1')

    await userEvent.click(screen.getByRole('button', { name: START }))

    expect(await screen.findByRole('alert')).toHaveTextContent(HUMAN_ERROR)
    expect(turnstile.reset).toHaveBeenCalledTimes(1)
    expect(screen.getByRole('button', { name: CHECKING })).toBeDisabled()
  })

  it('sends the fresh token on the retry after a failed attempt', async () => {
    api.createSession
      .mockRejectedValueOnce(new ApiError('turnstile_failed', 'Turnstile verification failed.', 403))
      .mockResolvedValueOnce(SUMMARY)
    renderGetStarted()
    solveChallenge('cf-token-1')
    await userEvent.click(screen.getByRole('button', { name: START }))
    await screen.findByRole('alert')

    solveChallenge('cf-token-2')
    await userEvent.click(screen.getByRole('button', { name: START }))

    expect(await screen.findByText('dashboard')).toBeInTheDocument()
    expect(api.createSession.mock.calls.map(([token]) => token)).toEqual(['cf-token-1', 'cf-token-2'])
    expect(turnstile.reset).toHaveBeenCalledTimes(2)
  })

  it('disables the button again when the token expires', () => {
    renderGetStarted()
    solveChallenge('cf-token-1')

    act(() => widget().onExpire?.())

    expect(screen.getByRole('button', { name: CHECKING })).toBeDisabled()
  })

  it('shows the friendly alert when the widget itself errors, and clears it once solved', async () => {
    renderGetStarted()

    act(() => widget().onError?.('300030'))

    expect(screen.getByRole('alert')).toHaveTextContent(HUMAN_ERROR)
    expect(screen.getByRole('button', { name: CHECKING })).toBeDisabled()

    solveChallenge('cf-token-1')
    await waitFor(() => expect(screen.queryByRole('alert')).not.toBeInTheDocument())
    expect(screen.getByRole('button', { name: START })).toBeEnabled()
  })
})
