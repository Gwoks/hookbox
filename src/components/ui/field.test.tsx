import { fireEvent, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { useState } from 'react'
import { describe, expect, it, vi } from 'vitest'
import { Field, Fieldset } from './field'
import { Input, TextField, Textarea } from './input'
import { Select, SelectField } from './select'
import { Slider } from './slider'
import { Switch, SwitchField } from './switch'

describe('Field + FieldContext', () => {
  it('auto-wires id and helper description into a child Input', () => {
    render(
      <Field label="Endpoint name" helper="Just a label for you.">
        <Input />
      </Field>,
    )
    const input = screen.getByLabelText('Endpoint name')
    expect(input.tagName).toBe('INPUT')
    expect(input).toHaveAccessibleDescription('Just a label for you.')
    expect(input).not.toHaveAttribute('aria-invalid')
  })

  it('an error replaces the helper, marks the control invalid and is announced', () => {
    render(
      <Field label="Email" helper="We never email you." error="Enter a valid email.">
        <Input />
      </Field>,
    )
    const input = screen.getByLabelText('Email')
    expect(input).toHaveAttribute('aria-invalid', 'true')
    expect(input).toHaveAccessibleDescription('Enter a valid email.')
    expect(screen.getByRole('alert')).toHaveTextContent('Enter a valid email.')
    expect(screen.queryByText('We never email you.')).toBeNull()
    expect(input).toHaveClass('border-danger-fg')
  })

  it('keeps the legacy render prop working', () => {
    render(
      <Field
        label="Target URL"
        helper="Where to forward."
        render={(p) => <Input id={p.id} aria-describedby={p.describedBy} invalid={p.invalid} />}
      />,
    )
    expect(screen.getByLabelText('Target URL')).toHaveAccessibleDescription('Where to forward.')
  })

  it('marks optional fields in the label', () => {
    render(
      <Field label="Label" optional>
        <Textarea />
      </Field>,
    )
    expect(screen.getByText('(optional)')).toBeInTheDocument()
    expect(screen.getByRole('textbox', { name: /Label/ }).tagName).toBe('TEXTAREA')
  })

  it('lets explicit props win over the context', () => {
    render(
      <Field label="Outer" id="outer">
        <Input id="explicit" aria-describedby="nothing" invalid />
      </Field>,
    )
    const input = screen.getByRole('textbox')
    expect(input).toHaveAttribute('id', 'explicit')
    expect(input).toHaveAttribute('aria-describedby', 'nothing')
    expect(input).toHaveAttribute('aria-invalid', 'true')
  })

  it('leaves a bare control untouched outside a Field', () => {
    render(<Input placeholder="free" />)
    const input = screen.getByPlaceholderText('free')
    expect(input).not.toHaveAttribute('id')
    expect(input).not.toHaveAttribute('aria-describedby')
  })
})

describe('TextField', () => {
  it('is a labelled input with helper and error', () => {
    render(<TextField label="Name" helper="Shown in the switcher." mono defaultValue="api" />)
    const input = screen.getByLabelText('Name')
    expect(input).toHaveValue('api')
    expect(input).toHaveClass('font-mono')
    expect(input).toHaveAccessibleDescription('Shown in the switcher.')
  })
})

describe('Select', () => {
  it('renders options, a decorative chevron, and reports changes', async () => {
    const onChange = vi.fn()
    render(
      <SelectField
        label="Chaos mode"
        helper="What a chaos hit does."
        value="error"
        onChange={(e) => onChange(e.target.value)}
        options={[
          { value: 'error', label: 'Return an error' },
          { value: 'dropout', label: 'Drop the connection' },
        ]}
      />,
    )
    const select = screen.getByRole('combobox', { name: 'Chaos mode' })
    expect(select).toHaveAccessibleDescription('What a chaos hit does.')
    await userEvent.selectOptions(select, 'dropout')
    expect(onChange).toHaveBeenCalledWith('dropout')
    expect(select.parentElement?.querySelector('svg')).toHaveAttribute('aria-hidden', 'true')
  })

  it('accepts children options and the invalid flag', () => {
    render(
      <Select aria-label="Method" invalid defaultValue="GET">
        <option value="GET">GET</option>
      </Select>,
    )
    expect(screen.getByRole('combobox', { name: 'Method' })).toHaveAttribute('aria-invalid', 'true')
  })
})

describe('Switch', () => {
  it('uses the thumb token, not a raw white', () => {
    render(<Switch aria-label="Auto-CRUD" />)
    const thumb = screen.getByRole('switch', { name: 'Auto-CRUD' }).firstElementChild
    expect(thumb).toHaveClass('bg-control-thumb')
    expect(thumb?.className).not.toMatch(/bg-white/)
  })

  it('SwitchField labels the switch, describes it and toggles', async () => {
    function Harness() {
      const [on, setOn] = useState(false)
      return <SwitchField label="Auto-CORS" helper="Answer preflights." checked={on} onCheckedChange={setOn} />
    }
    render(<Harness />)
    const sw = screen.getByRole('switch', { name: 'Auto-CORS' })
    expect(sw).toHaveAccessibleDescription('Answer preflights.')
    expect(sw).toHaveAttribute('aria-checked', 'false')
    await userEvent.click(screen.getByText('Auto-CORS'))
    expect(sw).toHaveAttribute('aria-checked', 'true')
  })
})

describe('Slider', () => {
  it('takes its id from a Field and reports numeric changes', () => {
    const onChange = vi.fn()
    render(
      <Field label="Latency">
        <Slider value={10} min={0} max={100} onChange={onChange} />
      </Field>,
    )
    const slider = screen.getByRole('slider', { name: 'Latency' })
    fireEvent.change(slider, { target: { value: '40' } })
    expect(onChange).toHaveBeenCalledWith(40)
    expect(slider).toHaveClass('bg-surface-active')
  })
})

describe('Fieldset', () => {
  it('groups controls under a legend with a description', () => {
    render(
      <Fieldset legend="Throttling" helper="Applies to every request.">
        <TextField label="Rate" />
      </Fieldset>,
    )
    const group = screen.getByRole('group', { name: 'Throttling' })
    expect(group).toHaveAccessibleDescription('Applies to every request.')
    expect(screen.getByLabelText('Rate')).toBeInTheDocument()
  })
})
