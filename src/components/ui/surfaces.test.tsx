import { render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { describe, expect, it } from 'vitest'
import { Card, CardHeader, cardVariants } from './card'
import { EmptyState } from './empty-state'
import { LoadingBlock } from './loading-block'
import { Overline } from './overline'
import { PageHeader } from './page-header'
import { Pill } from './pill'
import { MarketingSection, SettingsSection } from './section'

describe('Card', () => {
  it('is a hairline-bordered white surface with 24px padding by default', () => {
    render(<Card data-testid="card">body</Card>)
    const card = screen.getByTestId('card')
    expect(card).toHaveClass('rounded-md', 'border', 'border-border', 'bg-surface', 'p-6')
    expect(card.className).not.toMatch(/shadow/)
  })

  it.each(['marigold', 'coral', 'sky', 'tint', 'peach', 'midnight'] as const)(
    '%s tone paints the accent-card tokens',
    (tone) => {
      expect(cardVariants({ tone })).toContain(`bg-card-${tone}`)
      expect(cardVariants({ tone })).toContain(`text-card-${tone}-fg`)
      expect(cardVariants({ tone })).toContain(`border-card-${tone}-border`)
    },
  )

  it('supports padding variants and a semantic element', () => {
    render(
      <Card as="section" padding="none" aria-label="Feed">
        x
      </Card>,
    )
    const card = screen.getByRole('region', { name: 'Feed' })
    expect(card.tagName).toBe('SECTION')
    expect(card.className).not.toMatch(/\bp-\d/)
    expect(cardVariants({ padding: 'sm' })).toContain('p-4')
    expect(cardVariants({ padding: 'lg' })).toContain('p-8')
  })

  it('CardHeader renders a heading, description and actions', () => {
    render(
      <CardHeader title="Rules" description="Evaluated top to bottom." actions={<button>Add</button>} titleAs="h3" />,
    )
    expect(screen.getByRole('heading', { level: 3, name: 'Rules' })).toBeInTheDocument()
    expect(screen.getByText('Evaluated top to bottom.')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Add' })).toBeInTheDocument()
  })
})

describe('Pill', () => {
  it('is a 9999px pill with tone and size', () => {
    render(
      <Pill tone="success" size="md">
        Live
      </Pill>,
    )
    expect(screen.getByText('Live')).toHaveClass('rounded-pill', 'bg-success-bg', 'text-success-fg', 'px-3')
  })

  it('has a hero size for the headline highlight and a toneless option', () => {
    render(
      <>
        <Pill tone="peach" size="hero">
          watch
        </Pill>
        <Pill tone="none">plain</Pill>
      </>,
    )
    expect(screen.getByText('watch')).toHaveClass('bg-card-peach', 'px-6')
    expect(screen.getByText('plain').className).not.toMatch(/bg-/)
  })
})

describe('Overline', () => {
  it('renders uppercase overline text without extra tracking', () => {
    render(<Overline as="h3">Response</Overline>)
    const heading = screen.getByRole('heading', { level: 3, name: 'Response' })
    expect(heading).toHaveClass('text-overline', 'uppercase', 'text-text-tertiary')
    expect(heading.className).not.toMatch(/tracking/)
  })
})

describe('EmptyState', () => {
  it('renders a heading, body and action', () => {
    render(<EmptyState title="No requests yet" description="Send one to your mock URL." action={<a href="/cli">CLI</a>} />)
    expect(screen.getByRole('heading', { level: 2, name: 'No requests yet' })).toBeInTheDocument()
    expect(screen.getByText('Send one to your mock URL.')).toBeInTheDocument()
    expect(screen.getByRole('link', { name: 'CLI' })).toBeInTheDocument()
  })
})

describe('LoadingBlock', () => {
  it('is aria-busy with an sr-only label and decorative skeleton lines', () => {
    const { container } = render(<LoadingBlock label="Loading rules" lines={4} />)
    const block = container.firstElementChild as HTMLElement
    expect(block).toHaveAttribute('aria-busy', 'true')
    expect(screen.getByText('Loading rules')).toHaveClass('sr-only')
    expect(block.querySelectorAll('.skeleton')).toHaveLength(4)
  })
})

describe('sections', () => {
  it('SettingsSection is a titled card that clears the sticky nav on anchor jumps', () => {
    render(
      <SettingsSection id="cors" title="Auto-CORS" description="Answer preflights.">
        fields
      </SettingsSection>,
    )
    const section = screen.getByRole('region', { name: 'Auto-CORS' })
    expect(section).toHaveAttribute('id', 'cors')
    expect(section).toHaveClass('scroll-mt-nav', 'bg-surface', 'rounded-md')
    expect(section).toHaveAccessibleDescription('Answer preflights.')
  })

  it('SettingsSection supports the danger tone', () => {
    render(
      <SettingsSection title="Danger zone" tone="danger">
        x
      </SettingsSection>,
    )
    expect(screen.getByRole('region', { name: 'Danger zone' })).toHaveClass('border-danger-border')
  })

  it('MarketingSection has the 80px section rhythm', () => {
    render(
      <MarketingSection eyebrow="Why" title="Built for webhooks">
        x
      </MarketingSection>,
    )
    const section = screen.getByRole('region', { name: 'Built for webhooks' })
    expect(section).toHaveClass('py-20')
    expect(screen.getByText('Why')).toHaveClass('text-overline')
  })
})

describe('PageHeader', () => {
  const renderHeader = (ui: React.ReactElement) => render(<MemoryRouter>{ui}</MemoryRouter>)

  it('renders the h1, description, eyebrow, meta and actions', () => {
    renderHeader(
      <PageHeader
        title="Settings"
        description="How this endpoint answers."
        eyebrow="Endpoint"
        meta={<span>Mock URL</span>}
        actions={<button>Save</button>}
      />,
    )
    expect(screen.getByRole('heading', { level: 1, name: 'Settings' })).toHaveClass('text-h1')
    expect(screen.getByText('How this endpoint answers.')).toBeInTheDocument()
    expect(screen.getByText('Endpoint')).toBeInTheDocument()
    expect(screen.getByText('Mock URL')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Save' })).toBeInTheDocument()
  })

  it('names the back link with the REQUIRED explicit label', () => {
    renderHeader(<PageHeader title="Rule" back={{ to: '/d/ab12/rules', label: 'Back to rules' }} />)
    const back = screen.getByRole('link', { name: 'Back to rules' })
    expect(back).toHaveAttribute('href', '/d/ab12/rules')
  })

  it.each([
    ['doc', 'text-display-sm'],
    ['compact', 'text-h2'],
  ] as const)('size %s uses %s', (size, cls) => {
    renderHeader(<PageHeader title="T" size={size} />)
    expect(screen.getByRole('heading', { level: 1 })).toHaveClass(cls)
  })
})
