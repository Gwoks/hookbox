/** The feed's empty state: what to do next, plus a static, never-executed
 * curl sample against the endpoint's mock URL (copy-only). The URL itself
 * lives in the page header, so this block does not label it again. */
import { t } from '@/lib/copy'
import { absolutize } from '@/lib/url'
import { CodeBlock } from '@/components/hookbox/code-block'
import { EmptyState } from '@/components/ui/empty-state'
import { Overline } from '@/components/ui/overline'

export function FeedEmpty({ mockUrl }: { mockUrl: string }) {
  const sample = t('feed.empty.sample', { mock_url: absolutize(mockUrl) })
  return (
    <div className="px-4 pb-12 sm:px-6">
      <EmptyState titleAs="h3" title={t('feed.empty.title')} description={t('feed.empty.body')} className="pb-6" />
      <div className="mx-auto w-full max-w-md space-y-2">
        <Overline>{t('feed.empty.sampleLabel')}</Overline>
        <CodeBlock value={sample} ariaLabel={sample} />
        <p className="text-caption text-text-tertiary">{t('feed.empty.sampleHint')}</p>
      </div>
    </div>
  )
}
