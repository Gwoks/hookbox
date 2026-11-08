/**
 * PublicPage — the shell of every public page: skip link, PublicHeader (or a
 * custom/no header), an optional full-width banner, a width-capped <main
 * id="main"> and an optional footer aligned to the same column.
 */
import type { ReactNode } from 'react'
import { cn } from '@/lib/cn'
import { PublicHeader } from './public-header'
import { SkipLink } from './skip-link'

export type PublicPageWidth = 'page' | 'wide' | 'content' | 'doc'

const WIDTH_CLASSES: Record<PublicPageWidth, string> = {
  page: 'max-w-page',
  wide: 'max-w-wide',
  content: 'max-w-content',
  doc: 'max-w-doc',
}

export interface PublicPageProps {
  width?: PublicPageWidth
  /** `undefined` → the default PublicHeader; `null` → no header. */
  header?: ReactNode
  banner?: ReactNode
  footer?: ReactNode
  mainClassName?: string
  children: ReactNode
}

export function PublicPage({ width = 'content', header, banner, footer, mainClassName, children }: PublicPageProps) {
  const column = cn('mx-auto w-full px-4 sm:px-6', WIDTH_CLASSES[width])
  return (
    <div className="flex min-h-screen flex-col bg-canvas">
      <SkipLink />
      {header === undefined ? <PublicHeader /> : header}
      {banner}
      <main id="main" className={cn(column, 'flex-1 py-8', mainClassName)}>
        {children}
      </main>
      {footer && (
        <footer className="border-t border-border">
          <div className={cn(column, 'py-6 text-caption text-text-tertiary')}>{footer}</div>
        </footer>
      )}
    </div>
  )
}
