import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { Sheet, SheetContent, SheetTrigger } from './sheet'

describe('Sheet', () => {
  it('opens from its trigger as a named modal dialog on the right', async () => {
    const user = userEvent.setup()
    render(
      <Sheet>
        <SheetTrigger>Menu</SheetTrigger>
        <SheetContent title="Navigation" description="Jump to a screen.">
          <a href="/rules">Rules</a>
        </SheetContent>
      </Sheet>,
    )
    await user.click(screen.getByRole('button', { name: 'Menu' }))
    const dialog = screen.getByRole('dialog', { name: 'Navigation' })
    expect(dialog).toHaveAccessibleDescription('Jump to a screen.')
    expect(dialog).toHaveClass('right-0', 'inset-y-0')
    expect(screen.getByRole('link', { name: 'Rules' })).toBeInTheDocument()
  })

  it('docks to the bottom and closes through its close button', async () => {
    const user = userEvent.setup()
    const onOpenChange = vi.fn()
    render(
      <Sheet open onOpenChange={onOpenChange}>
        <SheetContent title="Inspector" side="bottom">
          detail
        </SheetContent>
      </Sheet>,
    )
    expect(screen.getByRole('dialog', { name: 'Inspector' })).toHaveClass('bottom-0', 'inset-x-0')
    await user.click(screen.getByRole('button', { name: 'Close' }))
    expect(onOpenChange).toHaveBeenCalledWith(false)
  })
})
