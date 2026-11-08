import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { useState } from 'react'
import { describe, expect, it } from 'vitest'
import { Input } from './input'
import { ListEditor, type ListEditorColumn } from './list-editor'

interface HeaderRow {
  uid: string
  key: string
  value: string
}

let nextUid = 0
const newRow = (): HeaderRow => ({ uid: `r${nextUid++}`, key: '', value: '' })

const COLUMNS: ListEditorColumn<HeaderRow>[] = [
  {
    key: 'key',
    header: 'Header',
    render: (row, { label, update }) => (
      <Input aria-label={label} value={row.key} onChange={(e) => update({ key: e.target.value })} />
    ),
  },
  {
    key: 'value',
    header: 'Value',
    width: '2fr',
    render: (row, { label, update }) => (
      <Input aria-label={label} value={row.value} onChange={(e) => update({ value: e.target.value })} />
    ),
  },
]

function Harness({ initial = [] as HeaderRow[] }) {
  const [rows, setRows] = useState<HeaderRow[]>(initial)
  return (
    <>
      <ListEditor
        legend="Response headers"
        helper="Sent with every matched response."
        rows={rows}
        columns={COLUMNS}
        onChange={setRows}
        createRow={newRow}
        addLabel="Add header"
      />
      <output data-testid="json">{JSON.stringify(rows.map(({ key, value }) => ({ key, value })))}</output>
    </>
  )
}

describe('ListEditor', () => {
  it('is a named group with a description and an empty state', () => {
    render(<Harness />)
    const group = screen.getByRole('group', { name: 'Response headers' })
    expect(group).toHaveAccessibleDescription('Sent with every matched response.')
    expect(within(group).getByText('Nothing added yet.')).toBeInTheDocument()
  })

  it('adds rows whose cells are labelled "{column} {n}"', async () => {
    const user = userEvent.setup()
    render(<Harness />)
    await user.click(screen.getByRole('button', { name: 'Add header' }))
    await user.click(screen.getByRole('button', { name: 'Add header' }))
    expect(screen.getByRole('textbox', { name: 'Header 1' })).toBeInTheDocument()
    expect(screen.getByRole('textbox', { name: 'Value 2' })).toBeInTheDocument()
    expect(screen.getAllByText('Header')).toHaveLength(1) // one visible column header
  })

  it('update(patch) changes only that cell of that row', async () => {
    const user = userEvent.setup()
    render(
      <Harness
        initial={[
          { uid: 'a', key: 'x-a', value: '1' },
          { uid: 'b', key: 'x-b', value: '2' },
        ]}
      />,
    )
    await user.type(screen.getByRole('textbox', { name: 'Value 2' }), '0')
    expect(screen.getByTestId('json')).toHaveTextContent('[{"key":"x-a","value":"1"},{"key":"x-b","value":"20"}]')
  })

  it('removes a row and renumbers the rest', async () => {
    const user = userEvent.setup()
    render(
      <Harness
        initial={[
          { uid: 'a', key: 'first', value: '' },
          { uid: 'b', key: 'second', value: '' },
        ]}
      />,
    )
    await user.click(screen.getByRole('button', { name: 'Remove row 1' }))
    expect(screen.getByRole('textbox', { name: 'Header 1' })).toHaveValue('second')
    expect(screen.queryByRole('textbox', { name: 'Header 2' })).toBeNull()
  })
})
