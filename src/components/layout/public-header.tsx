/**
 * PublicHeader — DESIGN.md's fixed top bar for the public pages (landing,
 * /cli, the /s/:code viewer): 64px, sticky, canvas background, the two-layer
 * nav shadow, a page-width row with the brand left, optional centered nav,
 * actions + theme toggle right. Viewer-safe: no owner client, no session.
 */
import type { ReactNode } from 'react'
import { Link } from 'react-router-dom'
import { t } from '@/lib/copy'
import { BrandMark } from '@/components/hookbox/brand-mark'
import { ThemeToggle } from '@/theme/theme'

export interface PublicHeaderProps {
  /** Replaces the default BrandMark. */
  brand?: ReactNode
  /** Wraps the brand in a link (omit on the page the brand would link to). */
  brandHref?: string
  nav?: ReactNode
  navLabel?: string
  actions?: ReactNode
}

export function PublicHeader({ brand = <BrandMark />, brandHref, nav, navLabel = t('layout.nav.aria'), actions }: PublicHeaderProps) {
  return (
    <header className="sticky top-0 z-nav h-nav bg-canvas shadow-nav">
      <div className="mx-auto flex h-full max-w-page items-center gap-4 px-4 sm:px-6">
        <div className="flex min-w-0 flex-1 items-center">
          {brandHref ? (
            <Link to={brandHref} className="rounded-sm">
              {brand}
            </Link>
          ) : (
            brand
          )}
        </div>
        {nav && (
          <nav aria-label={navLabel} className="hidden items-center gap-1 md:flex">
            {nav}
          </nav>
        )}
        <div className="flex flex-1 items-center justify-end gap-2">
          {actions}
          <ThemeToggle />
        </div>
      </div>
    </header>
  )
}
