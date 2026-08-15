/**
 * Landing / email gate (/) — PRD §3, AC-1(FE), AC-D22, AC-J1. The entry point;
 * anti-enumeration is both UX and security: the gate is visually AND copy
 * identical for a brand-new vs. existing email through submit/success — there is
 * NO "welcome back" string (copy.md §4.2 note, AC-D22).
 *
 * States (AC-J1): auto-resume (stored owner+token → redirect to /d/:token, no
 * form) · idle · submitting (landing.email.submitting, input disabled, button
 * loading) · field-error on 422 (landing.error.email.invalid) · 429 banner with
 * Retry-After (landing.error.rateLimit {seconds}) · network-error banner
 * (landing.error.network) · no-endpoint banner (landing.error.noEndpoint) ·
 * storage-unavailable warn (landing.warn.storage, still submits, no redirect
 * loop). Also surfaces the 401-bounce reason (common.error.401) carried in
 * router state.
 */
import { useEffect, useState } from 'react'
import { Link, useLocation, useNavigate } from 'react-router-dom'
import { api, ApiError, session, useSession } from '@/api'
import { t } from '@/lib/copy'
import { BrandMark } from '@/components/hookbox/brand-mark'
import { InlineAlert } from '@/components/hookbox/inline-alert'
import { Button } from '@/components/ui/button'
import { Field, Input } from '@/components/ui/input'
import { ThemeToggle } from '@/theme/theme'

const STRIP = [
  { title: t('landing.strip.mock.title'), body: t('landing.strip.mock.body') },
  { title: t('landing.strip.intercept.title'), body: t('landing.strip.intercept.body') },
  { title: t('landing.strip.inspect.title'), body: t('landing.strip.inspect.body') },
]

type Banner =
  | { kind: 'rateLimit'; seconds: number }
  | { kind: 'network' }
  | { kind: 'noEndpoint' }
  | { kind: 'bounced'; message: string }
  | null

export function Landing() {
  const navigate = useNavigate()
  const location = useLocation()
  const snap = useSession()

  const [email, setEmail] = useState('')
  const [submitting, setSubmitting] = useState(false)
  const [fieldError, setFieldError] = useState<string | null>(null)
  const [banner, setBanner] = useState<Banner>(null)

  // 401-bounce reason carried via router state (common.error.401).
  const bounceReason = (location.state as { reason?: string } | null)?.reason
  useEffect(() => {
    if (bounceReason) setBanner({ kind: 'bounced', message: bounceReason })
  }, [bounceReason])

  // Auto-resume: a stored owner secret means a live session — go straight to the
  // dashboard. We resolve the primary endpoint token from the endpoint list.
  const [resuming, setResuming] = useState(snap.hasSession)
  useEffect(() => {
    let cancelled = false
    if (!session.getSecret()) {
      setResuming(false)
      return
    }
    setResuming(true)
    api
      .listEndpoints()
      .then((eps) => {
        if (cancelled) return
        if (eps.length > 0) {
          navigate(`/d/${eps[0].token}`, { replace: true })
        } else {
          // Session exists but no endpoint — show the gate (no redirect loop).
          setResuming(false)
        }
      })
      .catch(() => {
        // A 401 already cleared the secret + bounced; otherwise show the gate.
        if (!cancelled) setResuming(false)
      })
    return () => {
      cancelled = true
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  if (resuming) {
    return (
      <div className="flex min-h-screen items-center justify-center bg-canvas" aria-busy="true">
        <span className="sr-only">{t('dash.state.loading.aria')}</span>
        <Button variant="ghost" loading aria-hidden="true" />
      </div>
    )
  }

  const storageWarn = !snap.storageAvailable

  async function onSubmit(e: React.FormEvent) {
    e.preventDefault()
    setFieldError(null)
    setBanner(null)
    setSubmitting(true)
    try {
      const res = await api.createSession({ email })
      // Persist the capability (rotate-overwrite). The active tab keeps it.
      session.set(res.owner_secret, res.owner_id, email)
      if (!res.primary || !res.primary.token) {
        setBanner({ kind: 'noEndpoint' })
        setSubmitting(false)
        return
      }
      navigate(`/d/${res.primary.token}`, { replace: true })
    } catch (err) {
      setSubmitting(false)
      if (err instanceof ApiError) {
        if (err.status === 422) {
          setFieldError(t('landing.error.email.invalid'))
          return
        }
        if (err.status === 429) {
          setBanner({ kind: 'rateLimit', seconds: err.retryAfter ?? 60 })
          return
        }
        if (err.status === 0 || err.code === 'network') {
          setBanner({ kind: 'network' })
          return
        }
      }
      setBanner({ kind: 'network' })
    }
  }

  return (
    <div className="flex min-h-screen flex-col bg-canvas">
      {/* Decorative teal glow — aria-hidden, reduced-motion-safe (design.md §8). */}
      <div
        aria-hidden="true"
        className="pointer-events-none fixed inset-x-0 top-0 -z-10 h-80"
        style={{
          background: 'radial-gradient(60% 100% at 50% 0%, var(--accent-subtle-bg), transparent)',
        }}
      />
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
              <h1 className="text-display text-text-primary">{t('landing.hero.headline')}</h1>
              <p className="text-body text-text-secondary">{t('landing.hero.subhead')}</p>
            </div>

            {banner?.kind === 'bounced' && (
              <InlineAlert variant="info" role="status">
                {banner.message}
              </InlineAlert>
            )}
            {banner?.kind === 'rateLimit' && (
              <InlineAlert variant="warning" role="alert">
                {t('landing.error.rateLimit', { seconds: banner.seconds })}
              </InlineAlert>
            )}
            {banner?.kind === 'network' && (
              <InlineAlert variant="danger" role="alert">
                {t('landing.error.network')}
              </InlineAlert>
            )}
            {banner?.kind === 'noEndpoint' && (
              <InlineAlert variant="danger" role="alert">
                {t('landing.error.noEndpoint')}
              </InlineAlert>
            )}
            {storageWarn && (
              <InlineAlert variant="info" role="status">
                {t('landing.warn.storage')}
              </InlineAlert>
            )}

            <form onSubmit={onSubmit} className="space-y-3" noValidate>
              <Field
                label={t('landing.email.label')}
                helper={t('landing.email.helper')}
                error={fieldError}
                render={(p) => (
                  <Input
                    id={p.id}
                    aria-describedby={p.describedBy}
                    invalid={p.invalid}
                    type="email"
                    inputMode="email"
                    autoComplete="email"
                    mono
                    placeholder={t('landing.email.placeholder')}
                    value={email}
                    disabled={submitting}
                    onChange={(e) => setEmail(e.target.value)}
                  />
                )}
              />
              <Button type="submit" variant="primary" size="lg" loading={submitting} className="w-full sm:w-auto">
                {submitting ? t('landing.email.submitting') : t('landing.email.submit')}
              </Button>
            </form>
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
