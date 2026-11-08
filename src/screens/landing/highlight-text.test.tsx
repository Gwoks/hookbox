import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { HighlightText } from './highlight-text'

describe('HighlightText', () => {
  it('wraps the first occurrence of the highlight in a peach hero pill', () => {
    render(
      <h1>
        <HighlightText text="Mock, intercept, and intercept again." highlight="intercept" />
      </h1>,
    )
    const heading = screen.getByRole('heading')
    expect(heading).toHaveTextContent('Mock, intercept, and intercept again.')
    const pills = heading.querySelectorAll('.rounded-pill')
    expect(pills).toHaveLength(1)
    expect(pills[0]).toHaveTextContent(/^intercept$/)
    expect(pills[0]).toHaveClass('bg-card-peach')
  })

  it('renders plain text when the highlight is missing or empty', () => {
    const { container, rerender } = render(<HighlightText text="Watch HTTP live." highlight="intercept" />)
    expect(container).toHaveTextContent('Watch HTTP live.')
    expect(container.querySelector('.rounded-pill')).toBeNull()
    rerender(<HighlightText text="Watch HTTP live." highlight="" />)
    expect(container.querySelector('.rounded-pill')).toBeNull()
  })
})
