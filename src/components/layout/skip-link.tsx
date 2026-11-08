/** SkipLink — the first focusable element on every page; visible only while
 * focused, jumps past the nav to the page's <main id="main">. */
import { t } from '@/lib/copy'

export function SkipLink({ targetId = 'main', label = t('shell.skipLink') }: { targetId?: string; label?: string }) {
  return (
    <a
      href={`#${targetId}`}
      className="sr-only focus:not-sr-only focus:fixed focus:left-3 focus:top-3 focus:z-tooltip focus:rounded-sm focus:bg-surface focus:px-3 focus:py-2 focus:text-body-sm focus:text-text-primary focus:shadow-md"
    >
      {label}
    </a>
  )
}
