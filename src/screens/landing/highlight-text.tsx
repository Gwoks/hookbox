/** DESIGN.md's "Hero Highlight Pill": one word of a headline set on a peach
 * pill. The pill inherits the headline's size and weight; only the first
 * occurrence is wrapped, punctuation right after it stays on the same line
 * (no orphaned comma when the headline wraps), and a missing word leaves
 * the text untouched. */
import { Pill } from '@/components/ui/pill'

export function HighlightText({ text, highlight }: { text: string; highlight: string }) {
  const index = highlight ? text.indexOf(highlight) : -1
  if (index < 0) return <>{text}</>
  const after = text.slice(index + highlight.length)
  const glued = /^\S*/.exec(after)?.[0] ?? ''
  return (
    <>
      {text.slice(0, index)}
      <span className="whitespace-nowrap">
        <Pill tone="peach" size="hero" className="px-[0.3em] py-0 font-bold leading-[1.15]">
          {highlight}
        </Pill>
        {glued}
      </span>
      {after.slice(glued.length)}
    </>
  )
}
