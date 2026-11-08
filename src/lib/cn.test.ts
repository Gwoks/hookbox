import { describe, expect, it } from 'vitest'
import { cn } from './cn'

describe('cn — tailwind-merge configured with the design tokens', () => {
  it('keeps a custom font-size token next to a text color (different groups)', () => {
    expect(cn('text-body-sm', 'text-text-primary')).toBe('text-body-sm text-text-primary')
    expect(cn('text-text-on-accent', 'text-body-sm')).toBe('text-text-on-accent text-body-sm')
  })

  it('resolves two custom font-size tokens last-wins', () => {
    expect(cn('text-h4', 'text-body-sm')).toBe('text-body-sm')
    expect(cn('text-caption', 'text-overline')).toBe('text-overline')
    expect(cn('text-mono-sm', 'text-mono')).toBe('text-mono')
  })

  it('resolves text colors last-wins without touching the font size', () => {
    expect(cn('text-body-sm text-text-secondary', 'text-text-primary')).toBe(
      'text-body-sm text-text-primary',
    )
  })

  it('treats custom shadows as one group', () => {
    expect(cn('shadow-sm', 'shadow-nav')).toBe('shadow-nav')
    expect(cn('shadow-mockup', 'shadow-none')).toBe('shadow-none')
  })

  it('treats the pill radius as a radius', () => {
    expect(cn('rounded-sm', 'rounded-pill')).toBe('rounded-pill')
    expect(cn('rounded-pill', 'rounded-xs')).toBe('rounded-xs')
  })

  it('merges custom max-width, duration, easing and z-index tokens', () => {
    expect(cn('max-w-page', 'max-w-doc')).toBe('max-w-doc')
    expect(cn('duration-fast', 'duration-base')).toBe('duration-base')
    expect(cn('ease-standard', 'ease-spring')).toBe('ease-spring')
    expect(cn('z-nav', 'z-dialog')).toBe('z-dialog')
  })

  it('knows the custom spacing keys', () => {
    expect(cn('h-9', 'h-nav')).toBe('h-nav')
    expect(cn('scroll-mt-4', 'scroll-mt-nav')).toBe('scroll-mt-nav')
  })

  it('still handles clsx inputs and plain conflicts', () => {
    expect(cn('px-2', false, undefined, ['px-4', { 'py-1': true }])).toBe('px-4 py-1')
  })
})
