import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { relTime } from './time'

const NOW = Date.parse('2026-11-08T12:00:00Z')

describe('relTime', () => {
  beforeEach(() => {
    vi.useFakeTimers()
    vi.setSystemTime(NOW)
  })
  afterEach(() => vi.useRealTimers())

  const ago = (ms: number) => new Date(NOW - ms).toISOString()

  it('returns an empty string for an unparseable timestamp', () => {
    expect(relTime('not-a-date')).toBe('')
  })

  it('formats seconds, minutes, hours and days at each boundary', () => {
    expect(relTime(ago(59_000))).toBe('59s')
    expect(relTime(ago(60_000))).toBe('1m')
    expect(relTime(ago(59 * 60_000))).toBe('59m')
    expect(relTime(ago(60 * 60_000))).toBe('1h')
    expect(relTime(ago(23 * 3_600_000))).toBe('23h')
    expect(relTime(ago(24 * 3_600_000))).toBe('1d')
  })

  it('clamps future timestamps to zero seconds', () => {
    expect(relTime(new Date(NOW + 5_000).toISOString())).toBe('0s')
  })
})
