/** MessagePage — a whole-page message (404, endpoint gone, endpoint load
 * failure) in the public chrome: skip link, the 64px PublicHeader whose
 * brand links home, and a centered FullPageMessage as the page's h1. */
import type { ReactNode } from 'react'
import { FullPageMessage } from '@/components/layout/full-page-message'
import { PublicHeader } from '@/components/layout/public-header'
import { PublicPage } from '@/components/layout/public-page'

export interface MessagePageProps {
  title: ReactNode
  description?: ReactNode
  actions?: ReactNode
}

export function MessagePage({ title, description, actions }: MessagePageProps) {
  return (
    <PublicPage header={<PublicHeader brandHref="/" />} mainClassName="flex flex-col">
      <FullPageMessage title={title} description={description} actions={actions} />
    </PublicPage>
  )
}
