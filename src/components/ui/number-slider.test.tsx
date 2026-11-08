import { fireEvent, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { useState } from 'react'
import { describe, expect, it, vi } from 'vitest'
import { Field } from './field'
import { NumberSlider } from './number-slider'

const BASE = { label: 'Latency', unit: 'ms', helper: 'Delay before responding.', min: 0, max: 1000 }

describe('NumberSlider', () => {
  it('labels BOTH the number input (htmlFor) and the slider (aria-labelledby)', () => {
    render(<NumberSlider {...BASE} value={250} onChange={() => {}} />)
    const input = screen.getByRole('spinbutton', { name: 'Latency' })
    const slider = screen.getByRole('slider', { name: 'Latency' })
    expect(input).toHaveValue(250)
    expect(slider).toHaveValue('250')
    expect(input).toHaveAccessibleDescription(/ms/)
    expect(input).toHaveAccessibleDescription(/Delay before responding\./)
  })

  it('clamps typed values into [min, max]; a cleared field means min', () => {
    const onChange = vi.fn()
    render(<NumberSlider {...BASE} min={5} value={10} onChange={onChange} />)
    const input = screen.getByRole('spinbutton', { name: 'Latency' })
    fireEvent.change(input, { target: { value: '99999' } })
    expect(onChange).toHaveBeenLastCalledWith(1000)
    fireEvent.change(input, { target: { value: '-5' } })
    expect(onChange).toHaveBeenLastCalledWith(5)
    fireEvent.change(input, { target: { value: '' } })
    expect(onChange).toHaveBeenLastCalledWith(5)
  })

  it('reports slider moves and keeps the thumb inside the range', () => {
    const onChange = vi.fn()
    render(<NumberSlider {...BASE} value={5000} onChange={onChange} />)
    const slider = screen.getByRole('slider', { name: 'Latency' })
    expect(slider).toHaveValue('1000')
    fireEvent.change(slider, { target: { value: '300' } })
    expect(onChange).toHaveBeenCalledWith(300)
  })

  it('nullable: an "Inherit" switch turns the value into null and back', async () => {
    function Harness({ initial }: { initial: number | null }) {
      const [value, setValue] = useState<number | null>(initial)
      return (
        <>
          <NumberSlider {...BASE} nullable value={value} onChange={setValue} />
          <output data-testid="value">{String(value)}</output>
        </>
      )
    }
    render(<Harness initial={null} />)
    const inherit = screen.getByRole('switch', { name: 'Inherit' })
    expect(inherit).toHaveAttribute('aria-checked', 'true')
    expect(screen.getByRole('spinbutton', { name: 'Latency' })).toBeDisabled()
    expect(screen.getByRole('slider', { name: 'Latency' })).toBeDisabled()

    await userEvent.click(inherit)
    expect(screen.getByTestId('value')).toHaveTextContent('0')
    expect(screen.getByRole('spinbutton', { name: 'Latency' })).toBeEnabled()

    fireEvent.change(screen.getByRole('spinbutton', { name: 'Latency' }), { target: { value: '400' } })
    await userEvent.click(inherit)
    expect(screen.getByTestId('value')).toHaveTextContent('null')
    await userEvent.click(inherit)
    expect(screen.getByTestId('value')).toHaveTextContent('400')
  })

  it('does not inherit an outer Field id', () => {
    render(
      <Field label="Outer" id="outer">
        <NumberSlider {...BASE} value={1} onChange={() => {}} />
      </Field>,
    )
    expect(screen.getByRole('spinbutton', { name: 'Latency' })).not.toHaveAttribute('id', 'outer')
    expect(screen.getByRole('slider', { name: 'Latency' })).not.toHaveAttribute('id', 'outer')
  })
})
