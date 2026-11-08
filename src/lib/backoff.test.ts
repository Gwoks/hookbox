import { describe, expect, it, vi } from 'vitest'
import { backoffDelay } from './backoff'

describe('backoffDelay', () => {
  it('doubles from the base and caps at max (the viewer poll policy)', () => {
    const policy = { base: 5000, max: 60_000 }
    expect([1, 2, 3, 4, 5].map((n) => backoffDelay(n, policy))).toEqual([10_000, 20_000, 40_000, 60_000, 60_000])
  })

  it('adds proportional jitter from the injected random source', () => {
    const policy = { base: 250, max: 8000, jitter: 0.3, maxExponent: 5 }
    expect(backoffDelay(1, policy, () => 0)).toBe(500)
    expect(backoffDelay(1, policy, () => 1)).toBe(650)
    expect(backoffDelay(2, policy, () => 0.5)).toBe(1000 + 150)
  })

  it('caps the exponent and still clamps jitter to max (the live-feed policy)', () => {
    const policy = { base: 250, max: 8000, jitter: 0.3, maxExponent: 5 }
    expect(backoffDelay(5, policy, () => 0)).toBe(8000)
    expect(backoffDelay(50, policy, () => 0.99)).toBe(8000)
  })

  it('never consults the random source without jitter', () => {
    const random = vi.fn(() => 0.5)
    backoffDelay(3, { base: 100, max: 10_000 }, random)
    expect(random).not.toHaveBeenCalled()
  })
})
