/**
 * CLI / tunnel doc page (/cli) — PRD §3, AC-J8/J9, copy.md §5.11. A public
 * docs-style page for the reverse-tunnel CLI (HookBox never runs it; this
 * page documents it). Signed in, the run command carries the primary
 * endpoint's token, and the recovery key (the tunnel's `--key`) stays masked
 * until Reveal fetches it from POST /api/session/key into local state only,
 * with a sharing warning; Hide drops it again. Signed out, the command keeps
 * `<endpoint>` / `<recovery-key>` placeholders — this page never redirects.
 * The header's "Back to dashboard" (signed in) replaces a link that was
 * labelled "Settings" but went to the dashboard.
 */
import { Eye, EyeOff } from 'lucide-react'
import { useState } from 'react'
import { Link } from 'react-router-dom'
import { api, useSessionGate } from '@/api'
import { errorMessage } from '@/lib/api-errors'
import { t, type CopyKey } from '@/lib/copy'
import { useAsyncAction } from '@/lib/hooks/use-async-action'
import { useResource } from '@/lib/hooks/use-resource'
import { CodeBlock } from '@/components/hookbox/code-block'
import { InlineAlert } from '@/components/hookbox/inline-alert'
import { PublicHeader } from '@/components/layout/public-header'
import { PublicPage } from '@/components/layout/public-page'
import { Button } from '@/components/ui/button'
import { Card, CardHeader } from '@/components/ui/card'
import { PageHeader } from '@/components/ui/page-header'

const BEHAVIOR: readonly CopyKey[] = [
  'cli.behavior.order',
  'cli.behavior.noTunnel',
  'cli.behavior.takeover',
  'cli.behavior.reconnect',
]

const MASKED_KEY = '•'.repeat(24)

/** The tunnel binary defaults --host to ws://localhost:8080; derive the real
 * host from this page's origin so the copied command reaches this deploy. */
const tunnelHost = () => window.location.origin.replace(/^http/, 'ws')

export function Cli() {
  // Bootstraps identity (and migrates a legacy secret) but never redirects.
  const isSignedIn = useSessionGate().status === 'signed-in'
  const summary = useResource(
    (signal) => (isSignedIn ? api.getSession({ signal }) : Promise.resolve(null)),
    [isSignedIn],
  )
  const [revealedKey, setRevealedKey] = useState<string | null>(null)
  const reveal = useAsyncAction(() => api.revealKey())

  // Nothing owner-specific survives a sign-out in another tab.
  const token = isSignedIn ? (summary.data?.primary?.token ?? null) : null
  const key = isSignedIn ? revealedKey : null
  const command = t('cli.command.template', {
    port: t('cli.command.portDefault'),
    token: token ?? '<endpoint>',
    key: isSignedIn ? (key ?? MASKED_KEY) : t('cli.key.placeholder'),
    host: tunnelHost(),
  })

  async function toggleReveal() {
    if (revealedKey) {
      setRevealedKey(null)
      return
    }
    const result = await reveal.run()
    if (result.ok) setRevealedKey(result.value.key)
  }

  const header = (
    <PublicHeader
      brandHref="/"
      actions={
        token && (
          <Button variant="nav" size="sm" asChild>
            <Link to={`/d/${token}`}>{t('cli.nav.dashboard')}</Link>
          </Button>
        )
      }
    />
  )

  return (
    <PublicPage width="doc" header={header} mainClassName="space-y-6 py-10">
      <PageHeader size="doc" title={t('cli.title')} description={t('cli.intro')} />

      <Card as="section" className="space-y-4">
        <CardHeader
          title={t('cli.command.label')}
          actions={
            isSignedIn && (
              <Button
                variant="tint"
                size="sm"
                onClick={() => void toggleReveal()}
                aria-pressed={key !== null}
                loading={reveal.pending}
              >
                {key ? <EyeOff className="h-3.5 w-3.5" aria-hidden="true" /> : <Eye className="h-3.5 w-3.5" aria-hidden="true" />}
                {key ? t('cli.key.hide') : t('cli.key.reveal')}
              </Button>
            )
          }
        />
        <CodeBlock value={command} ariaLabel={t('cli.command.copy.aria')} />
        {reveal.error !== null && (
          <InlineAlert variant="danger" role="alert">
            {errorMessage(reveal.error, t('cli.key.error'))}
          </InlineAlert>
        )}
        {isSignedIn && (
          <InlineAlert variant="warning" role="status">
            {t('cli.key.warning')}
          </InlineAlert>
        )}
      </Card>

      <Card as="section" className="space-y-4">
        <CardHeader title={t('cli.behavior.title')} />
        <ul className="list-disc space-y-2 pl-5 text-body-sm text-text-secondary marker:text-text-tertiary">
          {BEHAVIOR.map((item) => (
            <li key={item}>{t(item)}</li>
          ))}
        </ul>
      </Card>
    </PublicPage>
  )
}
