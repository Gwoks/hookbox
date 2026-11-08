/**
 * "How it works" — DESIGN.md's 2×2 accent grid: a full-width marigold Mock
 * card with a rule snippet, then sky Intercept and coral Inspect, then the
 * midnight "dark island" card for the tunnel CLI.
 */
import type { ReactNode } from 'react'
import { Link } from 'react-router-dom'
import { cn } from '@/lib/cn'
import { t } from '@/lib/copy'
import { Button } from '@/components/ui/button'
import { Card, type CardTone } from '@/components/ui/card'
import { MarketingSection } from '@/components/ui/section'

export const FEATURES_ID = 'features'

function Snippet({ children }: { children: string }) {
  return (
    <pre className="overflow-x-auto whitespace-pre-wrap break-all rounded-sm bg-surface px-3 py-2 font-mono text-mono text-text-primary">
      <code>{children}</code>
    </pre>
  )
}

function FeatureCard({
  tone,
  title,
  body,
  className,
  children,
}: {
  tone: CardTone
  title: string
  body: string
  className?: string
  children?: ReactNode
}) {
  return (
    <Card as="article" tone={tone} padding="lg" className={cn('flex flex-col gap-4', className)}>
      <div className="space-y-2">
        <h3 className="text-h3">{title}</h3>
        <p className="max-w-doc text-body">{body}</p>
      </div>
      {children}
    </Card>
  )
}

export function Features() {
  const tunnelCommand = t('landing.tunnel.command')
  return (
    <MarketingSection
      id={FEATURES_ID}
      eyebrow={t('landing.features.eyebrow')}
      title={t('landing.features.title')}
      description={t('landing.features.lead')}
      className="scroll-mt-nav"
    >
      <div className="mx-auto grid max-w-wide gap-4 md:grid-cols-2">
        <FeatureCard
          tone="marigold"
          title={t('landing.strip.mock.title')}
          body={t('landing.strip.mock.body')}
          className="md:col-span-2"
        >
          <Snippet>{t('landing.features.mock.sample')}</Snippet>
        </FeatureCard>
        <FeatureCard tone="sky" title={t('landing.strip.intercept.title')} body={t('landing.strip.intercept.body')} />
        <FeatureCard tone="coral" title={t('landing.strip.inspect.title')} body={t('landing.strip.inspect.body')} />
        <FeatureCard tone="midnight" title={t('cli.title')} body={t('landing.tunnel.body')} className="md:col-span-2">
          <Snippet>{tunnelCommand}</Snippet>
          <div>
            <Button variant="tint" asChild>
              <Link to="/cli">{t('landing.tunnel.link')}</Link>
            </Button>
          </div>
        </FeatureCard>
      </div>
    </MarketingSection>
  )
}
