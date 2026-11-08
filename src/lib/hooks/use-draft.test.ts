import { act, renderHook } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { shallowDiff, useDraft } from './use-draft'

interface Form {
  name: string
  latency: number
  headers: { key: string }[]
  target: string | null
}

const SOURCE: Form = { name: 'api', latency: 0, headers: [{ key: 'x' }], target: null }

describe('shallowDiff', () => {
  it('returns only the top-level keys whose values differ', () => {
    expect(shallowDiff(SOURCE, { ...SOURCE, latency: 5 })).toEqual({ latency: 5 })
    expect(shallowDiff(SOURCE, SOURCE)).toEqual({})
  })

  it('compares nested arrays/objects by value, not identity', () => {
    expect(shallowDiff(SOURCE, { ...SOURCE, headers: [{ key: 'x' }] })).toEqual({})
    expect(shallowDiff(SOURCE, { ...SOURCE, headers: [{ key: 'y' }] })).toEqual({ headers: [{ key: 'y' }] })
    expect(shallowDiff(SOURCE, { ...SOURCE, headers: [] })).toEqual({ headers: [] })
  })

  it('distinguishes null from a value and handles keys added on the draft', () => {
    expect(shallowDiff(SOURCE, { ...SOURCE, target: 'https://x' })).toEqual({ target: 'https://x' })
    const withExtra = { ...SOURCE, extra: 1 } as Form & { extra: number }
    expect(shallowDiff<Form & { extra?: number }>(SOURCE, withExtra)).toEqual({ extra: 1 })
  })
})

describe('useDraft', () => {
  it('starts clean from the source', () => {
    const { result } = renderHook(() => useDraft(SOURCE))
    expect(result.current.draft).toEqual(SOURCE)
    expect(result.current.dirty).toBe(false)
    expect(result.current.diff).toEqual({})
  })

  it('set / patch make it dirty with a minimal diff, without mutating the source', () => {
    const { result } = renderHook(() => useDraft(SOURCE))
    act(() => result.current.set('name', 'checkout'))
    act(() => result.current.patch({ latency: 250 }))
    expect(result.current.diff).toEqual({ name: 'checkout', latency: 250 })
    expect(result.current.dirty).toBe(true)
    expect(SOURCE.name).toBe('api')
  })

  it('editing back to the source value is clean again', () => {
    const { result } = renderHook(() => useDraft(SOURCE))
    act(() => result.current.set('latency', 9))
    act(() => result.current.set('latency', 0))
    expect(result.current.dirty).toBe(false)
  })

  it('reset discards edits', () => {
    const { result } = renderHook(() => useDraft(SOURCE))
    act(() => result.current.set('name', 'x'))
    act(() => result.current.reset())
    expect(result.current.draft).toEqual(SOURCE)
    expect(result.current.dirty).toBe(false)
  })

  it('rebases edits onto a refreshed source under the same resetKey (never reverts server changes)', () => {
    const { result, rerender } = renderHook(({ source, key }) => useDraft(source, { resetKey: key }), {
      initialProps: { source: SOURCE, key: 'ab12' },
    })
    act(() => result.current.set('name', 'edited'))
    rerender({ source: { ...SOURCE, latency: 100 }, key: 'ab12' })
    expect(result.current.draft.name).toBe('edited')
    // The untouched field follows the server; Save would not send latency: 0 back.
    expect(result.current.draft.latency).toBe(100)
    expect(result.current.diff).toEqual({ name: 'edited' })
  })

  it('a clean draft follows a refreshed source instead of turning dirty', () => {
    const { result, rerender } = renderHook(({ source }) => useDraft(source, { resetKey: 'ab12' }), {
      initialProps: { source: SOURCE },
    })
    rerender({ source: { ...SOURCE, target: 'https://up.example' } })
    expect(result.current.draft.target).toBe('https://up.example')
    expect(result.current.dirty).toBe(false)
  })

  it('an equal-by-value source object is not treated as a refresh', () => {
    const { result, rerender } = renderHook(({ source }) => useDraft(source), {
      initialProps: { source: SOURCE },
    })
    act(() => result.current.set('name', 'edited'))
    rerender({ source: { ...SOURCE, headers: [{ key: 'x' }] } })
    expect(result.current.draft.name).toBe('edited')
    expect(result.current.diff).toEqual({ name: 'edited' })
  })

  it('re-seeds from the source when resetKey changes', () => {
    const { result, rerender } = renderHook(({ source, key }) => useDraft(source, { resetKey: key }), {
      initialProps: { source: SOURCE, key: 'ab12' },
    })
    act(() => result.current.set('name', 'edited'))
    const other = { ...SOURCE, name: 'other' }
    rerender({ source: other, key: 'cd34' })
    expect(result.current.draft).toEqual(other)
    expect(result.current.dirty).toBe(false)
  })
})
