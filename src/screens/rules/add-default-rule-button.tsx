/** F6 "Add default rule" (design.md §3.8, AC-124): outline — "New rule"
 * owns the page's one primary. Disabled-with-a-reason needs both a mouse
 * and a keyboard path: a disabled <button> gets no pointer events and no
 * tab stop, so the Tooltip wraps a focusable <span> instead, with `title`
 * as the no-JS fallback. */
import { t } from '@/lib/copy'
import { Button } from '@/components/ui/button'
import { Tooltip } from '@/components/ui/tooltip'

export interface AddDefaultRuleButtonProps {
  isDisabled: boolean
  reason: string | null
  isAdding: boolean
  onClick: () => void
  className?: string
}

export function AddDefaultRuleButton({ isDisabled, reason, isAdding, onClick, className }: AddDefaultRuleButtonProps) {
  const button = (
    <Button
      variant="outline"
      className={className}
      disabled={isDisabled}
      loading={isAdding}
      onClick={onClick}
      aria-label={t('rules.default.aria')}
    >
      {isAdding ? t('rules.default.adding') : t('rules.default.add')}
    </Button>
  )
  if (!reason) return button
  return (
    <Tooltip content={reason}>
      <span tabIndex={0} title={reason} className="inline-flex rounded-sm">
        {button}
      </span>
    </Tooltip>
  )
}
