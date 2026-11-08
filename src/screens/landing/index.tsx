/**
 * Landing (/) — PRD §3, AC-1(FE), AC-J1, hookbox-a73.3. No email, no
 * account: identity is an anonymous recovery key the server keeps in an
 * HttpOnly cookie, and there is no "welcome back" copy anywhere (AC-D22).
 *
 * Session states: unknown → the gate asks the server (a loading or retry
 * frame); signed-in → resume straight to the primary endpoint; signed-out
 * → the page: hero (+ the mint / restore call to action), the product
 * mockup, the feature grid and the footer, in the public chrome.
 */
import { Link } from 'react-router-dom'
import { useSessionGate } from '@/api'
import { t } from '@/lib/copy'
import { PublicHeader } from '@/components/layout/public-header'
import { PublicPage } from '@/components/layout/public-page'
import { SessionPending } from '@/components/owner/require-session'
import { Button } from '@/components/ui/button'
import { Features, FEATURES_ID } from './features'
import { GET_STARTED_ID } from './get-started'
import { Hero } from './hero'
import { ProductMockup } from './product-mockup'
import { useAutoResume } from './use-auto-resume'

function LandingHeader() {
  return (
    <PublicHeader
      nav={
        <>
          <Button variant="nav" asChild>
            <a href={`#${FEATURES_ID}`}>{t('landing.nav.how')}</a>
          </Button>
          <Button variant="nav" asChild>
            <Link to="/cli">{t('landing.nav.cli')}</Link>
          </Button>
        </>
      }
      actions={
        <Button variant="tint" size="sm" asChild>
          <a href={`#${GET_STARTED_ID}`}>{t('landing.nav.getStarted')}</a>
        </Button>
      }
    />
  )
}

export function Landing() {
  const gate = useSessionGate()
  const hasResumeFailed = useAutoResume(gate.status)

  if (gate.status === 'unknown') {
    return <SessionPending error={gate.bootstrapError} onRetry={gate.retry} />
  }
  if (gate.status === 'signed-in' && !hasResumeFailed) {
    return <SessionPending error={null} onRetry={gate.retry} />
  }

  return (
    <PublicPage width="page" header={<LandingHeader />} footer={<p>{t('landing.footer.tagline')}</p>} mainClassName="py-0">
      <Hero hasResumeFailed={hasResumeFailed} areCookiesEnabled={gate.cookiesEnabled} />
      <ProductMockup />
      <Features />
    </PublicPage>
  )
}
