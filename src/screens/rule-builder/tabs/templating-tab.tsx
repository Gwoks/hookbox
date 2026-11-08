/** Templating — tag chips that insert into the response body at its last
 * caret (no live render: a documented non-goal), plus the honesty note that
 * unknown tags stay literal. */
import { t, type CopyKey } from '@/lib/copy'
import { Button } from '@/components/ui/button'
import { Overline } from '@/components/ui/overline'

const TEMPLATE_TAGS: readonly { group: CopyKey; tags: readonly CopyKey[] }[] = [
  { group: 'rule.tmpl.group.time', tags: ['rule.tmpl.tag.now.iso', 'rule.tmpl.tag.now.unix'] },
  { group: 'rule.tmpl.group.random', tags: ['rule.tmpl.tag.random.uuid', 'rule.tmpl.tag.random.int'] },
  {
    group: 'rule.tmpl.group.request',
    tags: [
      'rule.tmpl.tag.request.path',
      'rule.tmpl.tag.request.query',
      'rule.tmpl.tag.request.header',
      'rule.tmpl.tag.request.body',
    ],
  },
  { group: 'rule.tmpl.group.state', tags: ['rule.tmpl.tag.state'] },
]

export function TemplatingTab({ onInsert }: { onInsert: (tag: string) => void }) {
  return (
    <div className="space-y-5">
      <div className="space-y-1">
        <p className="text-body-sm text-text-secondary">{t('rule.tmpl.intro')}</p>
        <p className="text-caption text-text-tertiary">{t('rule.tmpl.honesty')}</p>
      </div>
      {TEMPLATE_TAGS.map(({ group, tags }) => (
        <section key={group} className="space-y-2">
          <Overline as="h3">{t(group)}</Overline>
          <div className="flex flex-wrap gap-2">
            {tags.map((key) => {
              const tag = t(key)
              return (
                <Button
                  key={key}
                  type="button"
                  variant="outline"
                  size="sm"
                  className="font-mono text-mono-sm"
                  onClick={() => onInsert(tag)}
                >
                  {tag}
                </Button>
              )
            })}
          </div>
        </section>
      ))}
    </div>
  )
}
