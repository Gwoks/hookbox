import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { ApiError } from '@/api/errors'
import { t } from '@/lib/copy'
import { CodeBlock, MockUrlChip } from './code-block'
import { ConfirmDialog } from './confirm-dialog'
import { ConnectionPill } from './connection-pill'
import { StatusCode } from './status-code'

describe('StatusCode', () => {
  it.each([
    [204, 'border-success-fg'],
    [302, 'border-info-fg'],
    [429, 'border-warning-fg'],
    [503, 'border-danger-fg'],
    [101, 'border-text-tertiary'],
  ])('%i carries its class underline (not hue alone)', (code, cls) => {
    render(<StatusCode code={code} />)
    expect(screen.getByText(String(code))).toHaveClass(cls, 'text-text-primary')
  })
})

describe('CodeBlock / MockUrlChip', () => {
  it('render copy-only mono text on the subtle surface (no link)', () => {
    render(
      <>
        <CodeBlock value="curl https://x/e/t" ariaLabel="Command" />
        <MockUrlChip url="https://t.hookbox.test" />
      </>,
    )
    expect(screen.getByLabelText('Command')).toHaveTextContent('curl https://x/e/t')
    expect(screen.getByText('curl https://x/e/t').parentElement).toHaveClass('bg-surface-subtle')
    expect(screen.getByText('https://t.hookbox.test').parentElement).toHaveClass('bg-surface-subtle')
    expect(screen.queryByRole('link')).toBeNull()
    expect(screen.getAllByRole('button', { name: t('common.copy') })).toHaveLength(2)
  })

  it('can hide the copy button', () => {
    render(<CodeBlock value="x" copy={false} />)
    expect(screen.queryByRole('button')).toBeNull()
  })
})

describe('ConnectionPill', () => {
  it('is a polite status whose text carries the state', () => {
    render(<ConnectionPill state="reconnecting" label="Reconnecting (2)…" />)
    const pill = screen.getByRole('status')
    expect(pill).toHaveTextContent('Reconnecting (2)…')
    expect(pill).toHaveClass('bg-surface-subtle', 'text-warning-fg')
  })
})

describe('ConfirmDialog', () => {
  function setup(onConfirm: () => Promise<void>) {
    const onClose = vi.fn()
    render(
      <ConfirmDialog
        open
        onClose={onClose}
        title="Delete rule?"
        body="This can't be undone."
        confirmLabel="Delete"
        errorFallback="Couldn't delete."
        onConfirm={onConfirm}
      />,
    )
    return { onClose }
  }

  it('closes after a successful confirm', async () => {
    const { onClose } = setup(async () => {})
    await userEvent.click(screen.getByRole('button', { name: 'Delete' }))
    await waitFor(() => expect(onClose).toHaveBeenCalledTimes(1))
  })

  it("stays open and shows the server's detail on failure", async () => {
    const { onClose } = setup(async () => {
      throw new ApiError('conflict', 'Rule is in use.', 409)
    })
    await userEvent.click(screen.getByRole('button', { name: 'Delete' }))
    expect(await screen.findByRole('alert')).toHaveTextContent('Rule is in use.')
    expect(onClose).not.toHaveBeenCalled()
  })

  it('falls back to the caller copy for a non-API failure, and Cancel closes', async () => {
    const { onClose } = setup(async () => {
      throw new Error('boom')
    })
    await userEvent.click(screen.getByRole('button', { name: 'Delete' }))
    expect(await screen.findByRole('alert')).toHaveTextContent("Couldn't delete.")
    await userEvent.click(screen.getByRole('button', { name: t('common.cancel') }))
    expect(onClose).toHaveBeenCalledTimes(1)
  })
})
