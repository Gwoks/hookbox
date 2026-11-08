import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { Button, buttonVariants } from './button'

describe('Button', () => {
  it('primary keeps BOTH its font size and its on-accent text color (cn regression)', () => {
    render(<Button>Save</Button>)
    const button = screen.getByRole('button', { name: 'Save' })
    expect(button).toHaveClass('bg-accent-fill', 'text-text-on-accent', 'text-body-sm', 'hover:bg-accent-hover')
    expect(button.className).not.toMatch(/opacity-90/)
  })

  it.each([
    ['tint', ['bg-accent-subtle-bg', 'text-accent-strong', 'hover:bg-accent-subtle-hover']],
    ['ghost', ['text-text-secondary', 'hover:bg-surface-hover']],
    ['outline', ['rounded-xs', 'border', 'border-border-strong', 'h-8']],
    ['danger', ['bg-danger-bg', 'text-danger-fg', 'border-danger-border']],
    ['danger-ghost', ['text-danger-fg', 'hover:bg-danger-bg']],
    ['nav', ['text-text-secondary', 'aria-[current=page]:text-text-primary']],
  ] as const)('%s variant uses its tokens', (variant, classes) => {
    render(<Button variant={variant}>Go</Button>)
    expect(screen.getByRole('button', { name: 'Go' })).toHaveClass(...classes)
  })

  it('danger is soft: never a filled red', () => {
    expect(buttonVariants({ variant: 'danger' })).not.toMatch(/bg-danger-fg/)
  })

  it('keeps `secondary` as a deprecated alias of outline', () => {
    expect(buttonVariants({ variant: 'secondary' })).toBe(buttonVariants({ variant: 'outline' }))
  })

  it('link has no box: no fixed height and no padding', () => {
    render(<Button variant="link">Docs</Button>)
    const link = screen.getByRole('button', { name: 'Docs' })
    expect(link).toHaveClass('h-auto', 'px-0')
    expect(link).not.toHaveClass('h-9')
  })

  it('nav exposes the current page through aria-current, never an underline', () => {
    render(
      <Button variant="nav" aria-current="page">
        Requests
      </Button>,
    )
    const nav = screen.getByRole('button', { name: 'Requests' })
    expect(nav).toHaveAttribute('aria-current', 'page')
    expect(nav.className).not.toMatch(/underline/)
  })

  it('loading disables the button, sets aria-busy and shows a spinner', async () => {
    const onClick = vi.fn()
    render(
      <Button loading onClick={onClick}>
        Saving
      </Button>,
    )
    const button = screen.getByRole('button', { name: /Saving/ })
    expect(button).toBeDisabled()
    expect(button).toHaveAttribute('aria-busy', 'true')
    expect(screen.getByRole('status', { name: 'Loading' })).toBeInTheDocument()
    await userEvent.click(button)
    expect(onClick).not.toHaveBeenCalled()
  })

  it('asChild renders the child element with the button classes', () => {
    render(
      <Button asChild variant="tint">
        <a href="/cli">CLI</a>
      </Button>,
    )
    const link = screen.getByRole('link', { name: 'CLI' })
    expect(link).toHaveClass('bg-accent-subtle-bg')
    expect(link).not.toHaveAttribute('disabled')
  })

  it('lets className override a conflicting size class', () => {
    render(<Button className="px-2">Tight</Button>)
    const button = screen.getByRole('button', { name: 'Tight' })
    expect(button).toHaveClass('px-2')
    expect(button).not.toHaveClass('px-4')
  })
})
