/**
 * RequestDetailTabs — Headers / Query / Body / Response for one captured
 * request, shared by the owner Inspector (which appends its Trace tab via
 * `extraTabs`) and the public viewer's row disclosure. Takes only the fields
 * both the owner and the public projections carry (types from
 * @/api/schemas), so it stays viewer-safe. Every value renders as text.
 */
import type { ReactNode } from 'react'
import type { PublicRequestDetail } from '@/api/schemas'
import { cn } from '@/lib/cn'
import { t } from '@/lib/copy'
import { Overline } from '@/components/ui/overline'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { JsonTree } from './json-tree'
import { KeyValueRows } from './key-value-rows'
import { ServedByChip } from './served-by-chip'

export type RequestDetailTabsData = Pick<
  PublicRequestDetail,
  'served_by' | 'request_headers' | 'query_params' | 'request_body' | 'response_headers' | 'response_body'
>

export interface ExtraTab {
  value: string
  label: string
  content: ReactNode
}

function BodyOrEmpty({ raw, empty }: { raw: string | null; empty: string }) {
  return raw ? <JsonTree raw={raw} /> : <p className="text-body-sm text-text-tertiary">{empty}</p>
}

function Block({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="space-y-1.5">
      <Overline as="h3">{title}</Overline>
      {children}
    </section>
  )
}

export function RequestDetailTabs({
  detail,
  extraTabs = [],
  defaultValue = 'headers',
  className,
  panelClassName = 'p-3',
}: {
  detail: RequestDetailTabsData
  extraTabs?: readonly ExtraTab[]
  defaultValue?: string
  className?: string
  panelClassName?: string
}) {
  return (
    <Tabs defaultValue={defaultValue} className={className}>
      <TabsList>
        <TabsTrigger value="headers">{t('insp.tab.headers')}</TabsTrigger>
        <TabsTrigger value="query">{t('insp.tab.query')}</TabsTrigger>
        <TabsTrigger value="body">{t('insp.tab.body')}</TabsTrigger>
        <TabsTrigger value="response">{t('insp.tab.response')}</TabsTrigger>
        {extraTabs.map((tab) => (
          <TabsTrigger key={tab.value} value={tab.value}>
            {tab.label}
          </TabsTrigger>
        ))}
      </TabsList>
      <div className={cn(panelClassName)}>
        <TabsContent value="headers">
          <KeyValueRows data={detail.request_headers} emptyLabel={t('insp.headers.empty')} />
        </TabsContent>
        <TabsContent value="query">
          <KeyValueRows data={detail.query_params} emptyLabel={t('insp.query.empty')} />
        </TabsContent>
        <TabsContent value="body">
          <BodyOrEmpty raw={detail.request_body} empty={t('insp.body.empty')} />
        </TabsContent>
        <TabsContent value="response" className="space-y-4">
          <Block title={t('insp.response.servedByLabel')}>
            <ServedByChip servedBy={detail.served_by} />
          </Block>
          <Block title={t('insp.response.headers')}>
            <KeyValueRows data={detail.response_headers} emptyLabel={t('insp.headers.empty')} />
          </Block>
          <Block title={t('insp.response.body')}>
            <BodyOrEmpty raw={detail.response_body} empty={t('insp.response.empty')} />
          </Block>
        </TabsContent>
        {extraTabs.map((tab) => (
          <TabsContent key={tab.value} value={tab.value} className="space-y-4">
            {tab.content}
          </TabsContent>
        ))}
      </div>
    </Tabs>
  )
}
