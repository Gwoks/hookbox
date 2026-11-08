import { clsx, type ClassValue } from 'clsx'
import { extendTailwindMerge } from 'tailwind-merge'
import {
  borderRadius,
  boxShadow,
  fontSize,
  maxWidth,
  spacing,
  transitionDuration,
  transitionTimingFunction,
  zIndex,
} from '@/design/tokens'

const keysOf = (scale: Record<string, unknown>): string[] => Object.keys(scale)

/**
 * tailwind-merge taught the custom theme keys. Unconfigured, it reads
 * `text-body-sm` as a text COLOR (any unknown `text-*` value is a color), so
 * `text-body-sm text-text-on-accent` lost one of the two; unknown shadow /
 * radius / width keys were never de-duplicated at all.
 */
const twMerge = extendTailwindMerge({
  extend: {
    theme: {
      spacing: keysOf(spacing),
      // Theme (not just the `rounded` group) so rounded-t-*/rounded-l-* agree.
      borderRadius: keysOf(borderRadius),
    },
    classGroups: {
      'font-size': [{ text: keysOf(fontSize) }],
      shadow: [{ shadow: keysOf(boxShadow) }],
      'max-w': [{ 'max-w': keysOf(maxWidth) }],
      duration: [{ duration: keysOf(transitionDuration) }],
      ease: [{ ease: keysOf(transitionTimingFunction) }],
      z: [{ z: keysOf(zIndex) }],
    },
  },
})

/** Merge Tailwind class names, resolving conflicts (last-wins). */
export function cn(...inputs: ClassValue[]): string {
  return twMerge(clsx(inputs))
}
