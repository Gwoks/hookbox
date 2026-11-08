import { act, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { t } from '@/lib/copy'
import { CopyButton } from './copy-button'
import { Progress } from './progress'
import { Segmented } from './segmented'

describe('Segmented', () => {
  it('is a radiogroup with token classes and reports the picked value', async () => {
    const onChange = vi.fn()
    render(
      <Segmented
        aria-label="View"
        value="pretty"
        onChange={onChange}
        options={[
          { value: 'pretty', label: 'Pretty' },
          { value: 'raw', label: 'Raw' },
        ]}
      />,
    )
    const group = screen.getByRole('radiogroup', { name: 'View' })
    expect(group).toHaveClass('bg-surface-subtle')
    const pretty = screen.getByRole('radio', { name: 'Pretty' })
    expect(pretty).toHaveAttribute('aria-checked', 'true')
    expect(pretty).toHaveClass('rounded-xs')
    await userEvent.click(screen.getByRole('radio', { name: 'Raw' }))
    expect(onChange).toHaveBeenCalledWith('raw')
  })
})

describe('Progress', () => {
  it('exposes a clamped determinate progressbar', () => {
    render(<Progress value={30} max={60} label="30 of 60" />)
    const bar = screen.getByRole('progressbar', { name: '30 of 60' })
    expect(bar).toHaveAttribute('aria-valuenow', '30')
    expect((bar.firstElementChild as HTMLElement).style.width).toBe('50%')
  })

  it('renders 0% for an empty total', () => {
    render(<Progress value={3} max={0} label="none" />)
    expect((screen.getByRole('progressbar').firstElementChild as HTMLElement).style.width).toBe('0%')
  })
})

describe('CopyButton', () => {
  afterEach(() => vi.useRealTimers())

  it('announces success only after the clipboard write resolves, then resets', async () => {
    const writeText = vi.fn().mockResolvedValue(undefined)
    Object.defineProperty(navigator, 'clipboard', { configurable: true, value: { writeText } })
    vi.useFakeTimers({ shouldAdvanceTime: true })
    render(<CopyButton value="abc" size="sm" />)
    await userEvent.click(screen.getByRole('button', { name: t('common.copy') }))
    expect(writeText).toHaveBeenCalledWith('abc')
    expect(screen.getByRole('button', { name: t('common.copied') })).toBeInTheDocument()
    expect(screen.getByRole('status')).toHaveTextContent(t('common.copy.announce'))
    act(() => vi.advanceTimersByTime(1700))
    expect(screen.getByRole('button', { name: t('common.copy') })).toBeInTheDocument()
  })

  it('shows a distinct failure state when the clipboard is unavailable', async () => {
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: { writeText: vi.fn().mockRejectedValue(new Error('insecure')) },
    })
    render(<CopyButton value="abc" />)
    await userEvent.click(screen.getByRole('button', { name: t('common.copy') }))
    expect(await screen.findByRole('button', { name: t('common.copy.failed') })).toHaveClass('text-danger-fg')
  })
})
