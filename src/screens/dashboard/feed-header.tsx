/**
 * The feed card's header row: "Live feed" (h2) with the count, the "N new"
 * flush button while paused, the Feed actions menu (Export CSV, then the
 * destructive Clear all — the trigger stays enabled even when both items
 * are disabled, so keyboard users can open it and hear why, AC-1) and
 * Pause/Resume.
 */
import { MoreHorizontal, Pause, Play } from 'lucide-react'
import { t } from '@/lib/copy'
import { Button } from '@/components/ui/button'
import { Menu, MenuContent, MenuItem, MenuSeparator, MenuTrigger } from '@/components/ui/menu'

export interface FeedHeaderProps {
  count: number | null
  isPaused: boolean
  newCount: number
  onFlush: () => void
  onTogglePause: () => void
  /** AC-76/AC-82: one enable predicate for both menu items. */
  actionsDisabled: boolean
  /** Exactly one reason (empty/busy/offline), or null. */
  actionsHint: string | null
  onExport: () => void
  onClearAll: () => void
}

export function FeedHeader({
  count,
  isPaused,
  newCount,
  onFlush,
  onTogglePause,
  actionsDisabled,
  actionsHint,
  onExport,
  onClearAll,
}: FeedHeaderProps) {
  return (
    <div className="flex flex-wrap items-center justify-between gap-2 border-b border-border px-4 py-3">
      <div className="flex min-w-0 items-baseline gap-2">
        <h2 className="text-h4 text-text-primary">{t('feed.header.title')}</h2>
        {count !== null && <span className="text-caption text-text-tertiary">{t('feed.count', { n: count })}</span>}
      </div>
      <div className="flex items-center gap-1.5">
        {isPaused && newCount > 0 && (
          <Button variant="tint" size="sm" onClick={onFlush} aria-label={t('feed.newCount.aria', { n: newCount })}>
            {t('feed.newCount', { n: newCount })}
          </Button>
        )}
        <Menu>
          <MenuTrigger asChild>
            <Button variant="ghost" size="icon-sm" aria-label={t('feed.actions.menu.aria')}>
              <MoreHorizontal className="h-4 w-4" aria-hidden="true" />
            </Button>
          </MenuTrigger>
          <MenuContent align="end">
            <MenuItem disabled={actionsDisabled} aria-label={t('feed.export.aria')} onSelect={onExport}>
              {t('feed.export')}
            </MenuItem>
            <MenuSeparator />
            <MenuItem destructive disabled={actionsDisabled} aria-label={t('feed.clearAll.aria')} onSelect={onClearAll}>
              {t('feed.clearAll')}
            </MenuItem>
            {actionsHint && <p className="px-2 py-1.5 text-caption text-text-tertiary">{actionsHint}</p>}
          </MenuContent>
        </Menu>
        <Button
          variant="ghost"
          size="sm"
          onClick={onTogglePause}
          aria-label={isPaused ? t('feed.resume.aria') : t('feed.pause.aria')}
          aria-pressed={isPaused}
        >
          {isPaused ? (
            <Play className="h-3.5 w-3.5" aria-hidden="true" />
          ) : (
            <Pause className="h-3.5 w-3.5" aria-hidden="true" />
          )}
          {isPaused ? t('feed.resume') : t('feed.pause')}
        </Button>
      </div>
    </div>
  )
}
