/**
 * The rules list, in server order (ORDER BY priority, id — never re-sorted
 * here): one flush card, an Overline column header (sm+), and per row the
 * priority, the name (opens the builder), the match, an "Enabled: {name}"
 * switch and the Edit / Duplicate / Delete menu. Below sm, priority and
 * match fold under the name.
 */
import { MoreHorizontal } from 'lucide-react'
import type { MockRule } from '@/api'
import { t } from '@/lib/copy'
import { MethodBadge } from '@/components/hookbox/method-badge'
import { Button } from '@/components/ui/button'
import { Card } from '@/components/ui/card'
import { Menu, MenuContent, MenuItem, MenuTrigger } from '@/components/ui/menu'
import { Overline } from '@/components/ui/overline'
import { Switch } from '@/components/ui/switch'

const ROW_GRID =
  'grid grid-cols-[minmax(0,1fr)_auto_auto] items-center gap-x-3 sm:grid-cols-[4rem_minmax(0,1fr)_minmax(0,1.2fr)_3rem_2rem]'

export interface RulesTableProps {
  rules: readonly MockRule[]
  onEdit: (rule: MockRule) => void
  onDuplicate: (rule: MockRule) => void
  onDelete: (rule: MockRule) => void
  onToggle: (rule: MockRule, enabled: boolean) => void
}

const ruleName = (rule: MockRule) => rule.name || t('rules.row.unnamed')

function RuleMatch({ rule, className }: { rule: MockRule; className?: string }) {
  return (
    <span className={className}>
      <MethodBadge method={rule.match.method} />
      <code className="truncate font-mono text-mono-sm text-text-secondary" title={rule.match.path}>
        {rule.match.path}
      </code>
    </span>
  )
}

export function RulesTable({ rules, onEdit, onDuplicate, onDelete, onToggle }: RulesTableProps) {
  return (
    <Card padding="none" className="overflow-hidden">
      <div className={`${ROW_GRID} hidden border-b border-border px-4 py-2.5 sm:grid`} aria-hidden="true">
        <Overline as="span">{t('rules.col.priority')}</Overline>
        <Overline as="span">{t('rules.col.name')}</Overline>
        <Overline as="span">{t('rules.col.match')}</Overline>
        <Overline as="span" className="text-center">
          {t('rules.col.enabled')}
        </Overline>
        <span />
      </div>
      <ul>
        {rules.map((rule) => (
          <li key={rule.id} className={`${ROW_GRID} border-b border-border px-4 py-3 last:border-b-0`}>
            <span className="tnum hidden font-mono text-mono-sm text-text-secondary sm:block">{rule.priority}</span>
            <div className="min-w-0 space-y-1">
              <button
                type="button"
                onClick={() => onEdit(rule)}
                className="block max-w-full truncate rounded-xs text-left text-body-sm font-medium text-text-primary hover:underline"
              >
                {ruleName(rule)}
              </button>
              <span className="flex min-w-0 items-center gap-2 sm:hidden">
                <span className="tnum font-mono text-mono-sm text-text-tertiary">{rule.priority}</span>
                <RuleMatch rule={rule} className="flex min-w-0 items-center gap-2" />
              </span>
            </div>
            <RuleMatch rule={rule} className="hidden min-w-0 items-center gap-2 sm:flex" />
            <span className="flex justify-center">
              <Switch
                checked={rule.enabled}
                onCheckedChange={(enabled) => onToggle(rule, enabled)}
                aria-label={t('rules.row.toggle.aria', { name: ruleName(rule) })}
              />
            </span>
            <Menu>
              <MenuTrigger asChild>
                <Button variant="ghost" size="icon-sm" aria-label={t('rules.row.menu.aria')}>
                  <MoreHorizontal className="h-4 w-4" aria-hidden="true" />
                </Button>
              </MenuTrigger>
              <MenuContent align="end">
                <MenuItem onSelect={() => onEdit(rule)}>{t('rules.row.edit')}</MenuItem>
                <MenuItem onSelect={() => onDuplicate(rule)}>{t('rules.row.duplicate')}</MenuItem>
                <MenuItem destructive onSelect={() => onDelete(rule)}>
                  {t('rules.row.delete')}
                </MenuItem>
              </MenuContent>
            </Menu>
          </li>
        ))}
      </ul>
    </Card>
  )
}
