/**
 * WCAG AA contrast guard over the REAL token values in src/globals.css. Parses
 * the top-level `:root` (light) and `.dark` blocks, resolves var() chains,
 * composites translucent colors over their backdrop, and asserts >= 4.5:1
 * for every text/background pair the primitives rely on. A token edit that
 * breaks AA fails here instead of in an audit.
 */
import { readFileSync } from 'node:fs'
import path from 'node:path'
import { describe, expect, it } from 'vitest'

type Rgba = [r: number, g: number, b: number, a: number]
type Vars = Record<string, string>

const AA = 4.5
// vitest stubs CSS imports (even ?raw), so read the file from the project root.
const CSS = readFileSync(path.join(process.cwd(), 'src', 'globals.css'), 'utf8')

function topLevelBlock(css: string, selector: string): Vars {
  const clean = css.replace(/\/\*[\s\S]*?\*\//g, '')
  const match = new RegExp(`(?:^|\\n)${selector.replace('.', '\\.')}\\s*\\{([^}]*)\\}`).exec(clean)
  if (!match) throw new Error(`no top-level ${selector} block in globals.css`)
  const vars: Vars = {}
  for (const decl of match[1].matchAll(/(--[\w-]+)\s*:\s*([^;]+);/g)) {
    vars[decl[1]] = decl[2].trim()
  }
  return vars
}

const LIGHT = topLevelBlock(CSS, ':root')
const DARK = { ...LIGHT, ...topLevelBlock(CSS, '.dark') }

function resolve(vars: Vars, name: string, seen: string[] = []): string {
  if (seen.includes(name)) throw new Error(`var() cycle: ${[...seen, name].join(' -> ')}`)
  const raw = vars[name]
  if (raw === undefined) throw new Error(`undefined token ${name}`)
  const ref = /^var\((--[\w-]+)\)$/.exec(raw)
  return ref ? resolve(vars, ref[1], [...seen, name]) : raw
}

function parseColor(value: string): Rgba {
  const hex = /^#([0-9a-f]{3}|[0-9a-f]{6})$/i.exec(value)
  if (hex) {
    const h = hex[1].length === 3 ? [...hex[1]].map((c) => c + c).join('') : hex[1]
    return [0, 2, 4].map((i) => parseInt(h.slice(i, i + 2), 16)).concat(1) as Rgba
  }
  const fn = /^rgba?\(([^)]+)\)$/.exec(value)
  if (fn) {
    const [r, g, b, a = '1'] = fn[1].split(',').map((p) => p.trim())
    return [Number(r), Number(g), Number(b), Number(a)]
  }
  if (value === 'transparent') return [0, 0, 0, 0]
  throw new Error(`unparseable color ${value}`)
}

function over([r, g, b, a]: Rgba, [br, bg, bb]: Rgba): Rgba {
  return [r * a + br * (1 - a), g * a + bg * (1 - a), b * a + bb * (1 - a), 1]
}

function luminance([r, g, b]: Rgba): number {
  const channel = (c: number) => {
    const s = c / 255
    return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4
  }
  return 0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b)
}

function ratio(a: Rgba, b: Rgba): number {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x)
  return (hi + 0.05) / (lo + 0.05)
}

/** Contrast of `fg` on `bg`, with `bg` composited over `backdrop` (the
 * opaque surface it sits on) and `fg` over the result. */
function contrast(vars: Vars, fg: string, bg: string, backdrop = '--bg-canvas'): number {
  const base = parseColor(resolve(vars, backdrop))
  const background = over(parseColor(resolve(vars, bg)), base)
  return ratio(over(parseColor(resolve(vars, fg)), background), background)
}

const CARD_TONES = ['marigold', 'coral', 'sky', 'tint', 'peach', 'midnight'] as const
const THEMES = [
  ['light', LIGHT],
  ['dark', DARK],
] as const

describe.each(THEMES)('%s theme meets WCAG AA (>= 4.5:1)', (_name, vars) => {
  it.each([
    ['--text-on-accent', '--accent-fill'],
    ['--text-on-accent', '--accent-hover'],
    ['--accent-strong', '--accent-subtle-bg'],
    ['--accent-strong', '--accent-subtle-hover'],
    ['--danger-fg', '--danger-bg'],
  ])('%s on %s', (fg, bg) => {
    expect(contrast(vars, fg, bg)).toBeGreaterThanOrEqual(AA)
    expect(contrast(vars, fg, bg, '--bg-surface')).toBeGreaterThanOrEqual(AA)
  })

  it.each(['--text-primary', '--text-secondary', '--text-tertiary', '--accent-strong'])(
    '%s on the canvas and on surfaces',
    (fg) => {
      expect(contrast(vars, fg, '--bg-canvas')).toBeGreaterThanOrEqual(AA)
      expect(contrast(vars, fg, '--bg-surface')).toBeGreaterThanOrEqual(AA)
    },
  )

  it.each(CARD_TONES)('%s accent card text', (tone) => {
    expect(contrast(vars, `--card-${tone}-fg`, `--card-${tone}`)).toBeGreaterThanOrEqual(AA)
  })
})

describe('token parser', () => {
  it('reads both theme blocks and resolves var() chains', () => {
    expect(resolve(LIGHT, '--bg-canvas')).toMatch(/^#/)
    expect(resolve(DARK, '--bg-canvas')).not.toBe(resolve(LIGHT, '--bg-canvas'))
  })

  it('composites translucent colors and rejects unknown formats', () => {
    expect(over([0, 0, 0, 0.5], [255, 255, 255, 1])).toEqual([127.5, 127.5, 127.5, 1])
    expect(parseColor('#fff')).toEqual([255, 255, 255, 1])
    expect(() => parseColor('hsl(0 0% 0%)')).toThrow(/unparseable/)
    expect(() => resolve({ '--a': 'var(--a)' }, '--a')).toThrow(/cycle/)
  })
})
