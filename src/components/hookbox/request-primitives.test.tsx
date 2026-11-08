import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import type { PublicRequestDetail } from '@/api/schemas'
import { t } from '@/lib/copy'
import { FeedRow } from './feed-row'
import { REQUEST_GRID, REQUEST_GRID_DISCLOSURE, RequestCells } from './request-cells'
import { RequestDetailTabs } from './request-detail-tabs'
import { RequestSubject } from './request-subject'
import { RetryAlert } from './retry-alert'

const DETAIL: PublicRequestDetail = {
  id: 7,
  method: 'post',
  path: '/orders',
  status_code: 201,
  served_by: 'crud',
  duration_ms: 12,
  timestamp: '2026-11-08T12:00:00Z',
  request_headers: { 'content-type': 'application/json' },
  query_params: {},
  request_body: '{"sku":"A1"}',
  response_headers: { 'x-trace': 'abc' },
  response_body: null,
}

describe('RequestCells', () => {
  it('renders the MethodBadge FIRST (e2e AC-D18 pins the leading column)', () => {
    const { container } = render(
      <div className={`grid ${REQUEST_GRID}`}>
        <RequestCells request={DETAIL} />
      </div>,
    )
    const grid = container.firstElementChild as HTMLElement
    expect(grid.firstElementChild).toHaveTextContent('POST')
    expect(grid.children).toHaveLength(6)
    expect(screen.getByText('/orders')).toHaveAttribute('title', '/orders')
    expect(screen.getByText('201')).toBeInTheDocument()
    expect(screen.getByText('12ms')).toBeInTheDocument()
  })

  it('can fold the meta cells into one wrapping cell below sm', () => {
    const { container } = render(
      <div className={`grid ${REQUEST_GRID_DISCLOSURE}`}>
        <RequestCells request={DETAIL} wrapMeta />
      </div>,
    )
    const grid = container.firstElementChild as HTMLElement
    expect(grid.children).toHaveLength(4)
    expect(grid.lastElementChild).toHaveClass('sm:contents')
  })

  it('is what FeedRow renders inside its option row', async () => {
    const onSelect = vi.fn()
    render(<FeedRow row={{ ...DETAIL, method: 'GET' }} onSelect={onSelect} />)
    const option = screen.getByRole('option')
    expect(option.firstElementChild).toHaveTextContent('GET')
    await userEvent.click(option)
    expect(onSelect).toHaveBeenCalledWith(7)
  })
})

describe('RequestSubject', () => {
  it('shows method, path, status and served-by', () => {
    render(<RequestSubject request={DETAIL} />)
    expect(screen.getByText('POST')).toBeInTheDocument()
    expect(screen.getByText('/orders')).toBeInTheDocument()
    expect(screen.getByText('201')).toBeInTheDocument()
    expect(screen.getByText('crud')).toBeInTheDocument()
  })
})

describe('RequestDetailTabs', () => {
  it('has Headers/Query/Body/Response tabs over a public or owner detail', async () => {
    const user = userEvent.setup()
    render(<RequestDetailTabs detail={DETAIL} />)
    const tabs = screen.getAllByRole('tab').map((tab) => tab.textContent)
    expect(tabs).toEqual([t('insp.tab.headers'), t('insp.tab.query'), t('insp.tab.body'), t('insp.tab.response')])
    expect(screen.getByText('content-type')).toBeInTheDocument()

    await user.click(screen.getByRole('tab', { name: t('insp.tab.query') }))
    expect(screen.getByRole('tabpanel')).toHaveTextContent(t('insp.query.empty'))

    await user.click(screen.getByRole('tab', { name: t('insp.tab.response') }))
    expect(screen.getByRole('heading', { level: 3, name: t('insp.response.servedByLabel') })).toBeInTheDocument()
    expect(screen.getByText(t('insp.response.empty'))).toBeInTheDocument()
    expect(screen.getByText('x-trace')).toBeInTheDocument()
  })

  it('appends extra tabs (the owner Inspector adds Trace)', async () => {
    const user = userEvent.setup()
    render(
      <RequestDetailTabs
        detail={DETAIL}
        extraTabs={[{ value: 'trace', label: 'Trace', content: <p>matched rule 3</p> }]}
      />,
    )
    await user.click(screen.getByRole('tab', { name: 'Trace' }))
    expect(screen.getByText('matched rule 3')).toBeInTheDocument()
  })
})

describe('RetryAlert', () => {
  it('is an alert with an outline Retry button', async () => {
    const onRetry = vi.fn()
    render(
      <RetryAlert title="Couldn't load rules" onRetry={onRetry}>
        Check your connection.
      </RetryAlert>,
    )
    expect(screen.getByRole('alert')).toHaveTextContent("Couldn't load rules")
    const retry = screen.getByRole('button', { name: 'Retry' })
    expect(retry).toHaveClass('border-border-strong')
    await userEvent.click(retry)
    expect(onRetry).toHaveBeenCalledTimes(1)
  })

  it('shows a busy retry and omits the button without a handler', () => {
    const { rerender } = render(<RetryAlert title="x" onRetry={() => {}} retrying />)
    expect(screen.getByRole('button', { name: /Retry/ })).toHaveAttribute('aria-busy', 'true')
    rerender(<RetryAlert title="x" />)
    expect(screen.queryByRole('button')).toBeNull()
  })
})
