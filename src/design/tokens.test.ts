import { describe, expect, it } from 'vitest'
import {
  borderRadius,
  boxShadow,
  fontSize,
  maxWidth,
  spacing,
  transitionDuration,
  transitionTimingFunction,
  zIndex,
} from './tokens'

describe('design tokens', () => {
  it('follows the DESIGN.md type scale adapted to app density', () => {
    expect(fontSize['display-lg'][0]).toBe('72px')
    expect(fontSize.display).toEqual(['54px', expect.objectContaining({ letterSpacing: '-0.035em' })])
    expect(fontSize['display-sm'][0]).toBe('40px')
    expect(fontSize.h1[0]).toBe('24px')
    expect(fontSize.h2[1]).toEqual(expect.objectContaining({ lineHeight: '1.27', letterSpacing: '-0.011em' }))
    expect(fontSize.h4[0]).toBe('16px')
    expect(fontSize.lead).toEqual(['18px', expect.objectContaining({ lineHeight: '1.56' })])
    expect(fontSize.body).toEqual(['16px', expect.objectContaining({ lineHeight: '1.5' })])
    expect(fontSize['body-sm'][0]).toBe('14px')
    expect(fontSize.caption[1]).toEqual(expect.objectContaining({ letterSpacing: '0.01em' }))
    expect(fontSize.overline[1]).toEqual(expect.objectContaining({ letterSpacing: '0.06em' }))
  })

  it('has negative tracking on every display size and none on body sizes', () => {
    for (const key of ['display-lg', 'display', 'display-sm'] as const) {
      expect(fontSize[key][1].letterSpacing).toMatch(/^-/)
    }
    expect(fontSize.body[1].letterSpacing).toBeUndefined()
    expect(fontSize['body-sm'][1].letterSpacing).toBeUndefined()
  })

  it('caps rectangular radii at 12px and has no lg tier', () => {
    expect(borderRadius).toEqual({ xs: '4px', sm: '8px', md: '12px', pill: '9999px' })
  })

  it('exposes the layout widths and the nav height', () => {
    expect(maxWidth).toMatchObject({
      page: '1440px',
      wide: '1120px',
      content: '920px',
      doc: '720px',
      form: '560px',
    })
    expect(spacing).toEqual({ nav: '64px' })
  })

  it('routes theme-dependent values through CSS custom properties', () => {
    expect(boxShadow.nav).toBe('var(--shadow-nav)')
    expect(boxShadow.mockup).toBe('var(--shadow-mockup)')
    expect(transitionDuration.base).toBe('var(--dur-base)')
    expect(transitionTimingFunction.spring).toBe('var(--ease-spring)')
  })

  it('orders floating layers above sticky chrome', () => {
    expect(Number(zIndex.nav)).toBeGreaterThan(Number(zIndex.sticky))
    expect(Number(zIndex.dialog)).toBeGreaterThan(Number(zIndex.dropdown))
    expect(Number(zIndex.tooltip)).toBeGreaterThan(Number(zIndex.toast))
  })
})
