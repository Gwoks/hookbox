/** NotFound — the `*` route (and /_gallery in production builds). */
import { Link } from 'react-router-dom'
import { t } from '@/lib/copy'
import { Button } from '@/components/ui/button'
import { MessagePage } from './message-page'

export function NotFound() {
  return (
    <MessagePage
      title={t('common.notFound.title')}
      description={t('common.notFound.body')}
      actions={
        <Button variant="tint" asChild>
          <Link to="/">{t('common.notFound.home')}</Link>
        </Button>
      }
    />
  )
}
