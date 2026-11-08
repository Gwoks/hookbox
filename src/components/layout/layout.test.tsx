import { render, screen, within } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { describe, expect, it } from 'vitest'
import { FullPageMessage } from './full-page-message'
import { PublicHeader } from './public-header'
import { PublicPage } from './public-page'
import { SkipLink } from './skip-link'

const inRouter = (ui: React.ReactElement) => render(<MemoryRouter>{ui}</MemoryRouter>)

describe('SkipLink', () => {
  it('jumps to #main and only shows on focus', () => {
    render(<SkipLink />)
    const link = screen.getByRole('link', { name: 'Skip to content' })
    expect(link).toHaveAttribute('href', '#main')
    expect(link).toHaveClass('sr-only', 'focus:not-sr-only')
  })
})

describe('PublicHeader', () => {
  it('is the 64px sticky canvas bar with the nav shadow and a page-width row', () => {
    inRouter(<PublicHeader actions={<a href="/cli">CLI</a>} />)
    const header = screen.getByRole('banner')
    expect(header).toHaveClass('sticky', 'top-0', 'h-nav', 'bg-canvas', 'shadow-nav', 'z-nav')
    expect(header.firstElementChild).toHaveClass('max-w-page')
    expect(within(header).getByRole('img', { name: 'HookBox' })).toBeInTheDocument()
    expect(within(header).getByRole('link', { name: 'CLI' })).toBeInTheDocument()
    expect(within(header).getByRole('button', { name: /theme/i })).toBeInTheDocument()
    expect(within(header).queryByRole('navigation')).toBeNull()
  })

  it('renders a labelled nav and an optional home link on the brand', () => {
    inRouter(<PublicHeader brandHref="/" nav={<a href="/docs">Docs</a>} navLabel="Site" />)
    expect(screen.getByRole('navigation', { name: 'Site' })).toContainElement(screen.getByRole('link', { name: 'Docs' }))
    expect(screen.getByRole('link', { name: /HookBox/ })).toHaveAttribute('href', '/')
  })
})

describe('PublicPage', () => {
  it('composes skip link, header, banner, a width-capped main and footer', () => {
    inRouter(
      <PublicPage width="doc" banner={<p>Read-only</p>} footer={<p>Made with HookBox</p>}>
        <h1>CLI</h1>
      </PublicPage>,
    )
    expect(screen.getByRole('link', { name: 'Skip to content' })).toBeInTheDocument()
    expect(screen.getByRole('banner')).toBeInTheDocument()
    expect(screen.getByText('Read-only')).toBeInTheDocument()
    const main = screen.getByRole('main')
    expect(main).toHaveAttribute('id', 'main')
    expect(main).toHaveClass('max-w-doc')
    expect(within(main).getByRole('heading', { name: 'CLI' })).toBeInTheDocument()
    expect(screen.getByRole('contentinfo')).toHaveTextContent('Made with HookBox')
  })

  it.each([
    ['page', 'max-w-page'],
    ['wide', 'max-w-wide'],
    ['content', 'max-w-content'],
  ] as const)('width %s caps main at %s', (width, cls) => {
    inRouter(<PublicPage width={width}>x</PublicPage>)
    expect(screen.getByRole('main')).toHaveClass(cls)
  })

  it('can drop the default header', () => {
    inRouter(<PublicPage header={null}>x</PublicPage>)
    expect(screen.queryByRole('banner')).toBeNull()
  })
})

describe('FullPageMessage', () => {
  it('centers an h1, body and actions', () => {
    inRouter(
      <FullPageMessage
        title="Page not found"
        description="That page doesn't exist."
        actions={<a href="/">Back to start</a>}
      />,
    )
    expect(screen.getByRole('heading', { level: 1, name: 'Page not found' })).toBeInTheDocument()
    expect(screen.getByText("That page doesn't exist.")).toBeInTheDocument()
    expect(screen.getByRole('link', { name: 'Back to start' })).toBeInTheDocument()
  })
})
