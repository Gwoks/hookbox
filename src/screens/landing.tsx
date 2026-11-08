/**
 * Landing (/) — PRD §3, AC-1(FE), AC-J1, hookbox-a73.3. The entry point.
 * There is no email and no account: identity is an anonymous recovery key
 * the server keeps in an HttpOnly cookie. There is also no "welcome back"
 * copy anywhere (AC-D22).
 *
 * States:
 *   unknown            → the session gate asks the server (loading frame, or
 *                        a retry frame when it can't be reached)
 *   signed-in          → resume: straight to the primary endpoint (one is
 *                        created first if the owner has none left)
 *   signed-out         → "Get my endpoint" mints an anonymous owner + primary
 *                        endpoint and opens it with `{ fresh: true }` (the
 *                        dashboard then nudges them to save the key);
 *                        "I have a recovery key" reveals the restore form
 * Banners: the 401-bounce reason carried in router state (common.error.401),
 * rate limit (Retry-After), network, and a warning when the browser blocks
 * cookies (nothing could keep the tab signed in).
 */
import { useEffect, useRef, useState } from 'react'
import { Link, useLocation, useNavigate } from 'react-router-dom'
import { api, ApiError, resolvePrimaryToken, session, useSessionGate, type SessionStatus } from '@/api'
import { signInErrorMessage } from '@/lib/api-errors'
import { t } from '@/lib/copy'
import { useAsyncAction } from '@/lib/hooks/use-async-action'
import { BrandMark } from '@/components/hookbox/brand-mark'
import { InlineAlert } from '@/components/hookbox/inline-alert'
import { RestoreKeyForm } from '@/components/owner/restore-key-form'
import { SessionPending } from '@/components/owner/require-session'
import { Button } from '@/components/ui/button'
import { ThemeToggle } from '@/theme/theme'

const STRIP = [
  { title: t('landing.strip.mock.title'), body: t('landing.strip.mock.body') },
  { title: t('landing.strip.intercept.title'), body: t('landing.strip.intercept.body') },
  { title: t('landing.strip.inspect.title'), body: t('landing.strip.inspect.body') },
]

/**
 * When the gate resolves to signed-in (a returning visitor, or another tab
 * signing this browser in), open the owner's primary endpoint. A
 * signed-out → signed-in flip is this tab's own mint/restore, which marks
 * the tab signed in right before navigating itself, so it is skipped.
 * Returns true once the resume failed (the page then shows a banner).
 */
function useResumeSignedIn(status: SessionStatus): boolean {
  const navigate = useNavigate()
  const previous = useRef<SessionStatus | null>(null)
  const [failed, setFailed] = useState(false)

  useEffect(() => {
    const before = previous.current
    previous.current = status
    if (status !== 'signed-in' || before === 'signed-out') return
    let cancelled = false
    setFailed(false)
    api
      .getSession()
      .then(resolvePrimaryToken)
      .then((token) => {
        if (!cancelled) navigate(`/d/${token}`, { replace: true })
      })
      .catch((err: unknown) => {
        if (cancelled) return
        if (err instanceof ApiError && err.status === 401) session.markSignedOut()
        else setFailed(true)
      })
    return () => {
      cancelled = true
    }
  }, [status, navigate])

  return failed
}

export function Landing() {
  const navigate = useNavigate()
  const location = useLocation()
  const gate = useSessionGate()
  const resumeFailed = useResumeSignedIn(gate.status)
  const [restoreOpen, setRestoreOpen] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const start = useAsyncAction(async () => {
    const summary = await api.createSession()
    const token = await resolvePrimaryToken(summary)
    return { ownerId: summary.owner_id, token }
  })

  const bounceReason = (location.state as { reason?: unknown } | null)?.reason

  if (gate.status === 'unknown') {
    return <SessionPending error={gate.bootstrapError} onRetry={gate.retry} />
  }
  if (gate.status === 'signed-in' && !resumeFailed) {
    return <SessionPending error={null} onRetry={gate.retry} />
  }

  async function handleStart() {
    setError(null)
    const result = await start.run()
    if (!result.ok) {
      setError(signInErrorMessage(result.error))
      return
    }
    session.markSignedIn(result.value.ownerId, { broadcast: true })
    navigate(`/d/${result.value.token}`, { replace: true, state: { fresh: true } })
  }

  return (
    <div className="flex min-h-screen flex-col bg-canvas">
      <a href="#main" className="sr-only focus:not-sr-only focus:absolute focus:left-2 focus:top-2">
        {t('shell.skipLink')}
      </a>
      <header className="flex items-center justify-between px-6 py-4">
        <BrandMark />
        <div className="flex items-center gap-2">
          <Button variant="link" asChild>
            <Link to="/cli">{t('landing.cli.link')}</Link>
          </Button>
          <ThemeToggle />
        </div>
      </header>

      <main id="main" className="flex flex-1 flex-col items-center px-4 pb-16 pt-6 sm:pt-12">
        <div className="w-full max-w-landing-hero">
          <div className="mx-auto max-w-landing space-y-5 rounded-md border border-border bg-surface p-6 sm:p-8">
            <div className="space-y-2">
              <h1 className="text-display-sm text-text-primary sm:text-display">{t('landing.hero.headline')}</h1>
              <p className="text-body text-text-secondary">{t('landing.hero.subhead')}</p>
            </div>

            {typeof bounceReason === 'string' && (
              <InlineAlert variant="info" role="status">
                {bounceReason}
              </InlineAlert>
            )}
            {(error || resumeFailed) && (
              <InlineAlert variant="danger" role="alert">
                {error ?? t('landing.error.network')}
              </InlineAlert>
            )}
            {!gate.cookiesEnabled && (
              <InlineAlert variant="warning" role="status">
                {t('landing.warn.cookies')}
              </InlineAlert>
            )}

            <div className="space-y-3">
              <div className="flex flex-wrap gap-2">
                <Button variant="primary" size="lg" loading={start.pending} onClick={() => void handleStart()}>
                  {start.pending ? t('landing.start.submitting') : t('landing.start.submit')}
                </Button>
                <Button
                  variant="tint"
                  size="lg"
                  aria-expanded={restoreOpen}
                  onClick={() => setRestoreOpen((open) => !open)}
                >
                  {t('landing.restore.toggle')}
                </Button>
              </div>
              <p className="text-body-sm text-text-tertiary">{t('landing.start.helper')}</p>
            </div>

            {restoreOpen && <RestoreKeyForm />}
          </div>

          <ul className="mx-auto mt-10 grid max-w-landing-hero gap-4 sm:grid-cols-3">
            {STRIP.map((f) => (
              <li key={f.title} className="rounded-md border border-border bg-surface p-4">
                <h2 className="text-h4 text-text-primary">{f.title}</h2>
                <p className="mt-1 text-body-sm text-text-secondary">{f.body}</p>
              </li>
            ))}
          </ul>
        </div>
      </main>

      <footer className="px-6 py-6 text-center text-caption text-text-tertiary">
        {t('landing.footer.tagline')}
      </footer>
    </div>
  )
}
