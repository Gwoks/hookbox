/**
 * Configuration (F3): export this endpoint's settings + rules as a JSON
 * bundle, or import one. Export always builds from freshly fetched server
 * state, never the form (AC-87 — the caption says so while it is dirty).
 * Import is a visually-hidden file input with a button-styled <label>
 * (keyboard focus shows on the label via `peer-focus-visible`, AC-92).
 */
import { Link } from 'react-router-dom'
import { z } from 'zod'
import { api, mockRuleSchema, type EndpointDetail } from '@/api'
import { isEndpointGone } from '@/lib/api-errors'
import { cn } from '@/lib/cn'
import { buildBundle } from '@/lib/config-bundle'
import { t } from '@/lib/copy'
import { downloadBlob } from '@/lib/download'
import { useAsyncAction } from '@/lib/hooks/use-async-action'
import { ConfirmDialog } from '@/components/hookbox/confirm-dialog'
import { InlineAlert } from '@/components/hookbox/inline-alert'
import { Button, buttonVariants } from '@/components/ui/button'
import { Progress } from '@/components/ui/progress'
import { SettingsSection } from '@/components/ui/section'
import { useToast } from '@/components/ui/toast'
import { ImportConfirmBody } from './import-confirm-body'
import { useConfigImport } from './use-config-import'

const IMPORT_INPUT_ID = 'cfg-import'
const FILE_LABEL_CLASSES = cn(buttonVariants({ variant: 'outline' }), 'cursor-pointer')

class ExportError extends Error {}

async function exportConfig(token: string): Promise<void> {
  const fresh = await api.getEndpoint(token).catch((err: unknown) => {
    throw new ExportError(isEndpointGone(err) ? t('common.error.endpointGone') : t('set.config.export.error'))
  })
  const rules = await api
    .listRules(token)
    .then((list) => z.array(mockRuleSchema).parse(list))
    .catch(() => {
      throw new ExportError(t('set.config.export.error.rules'))
    })
  const bundle = buildBundle(fresh, rules)
  downloadBlob(`hookbox-config-${token}.json`, 'application/json', JSON.stringify(bundle, null, 2))
}

export interface ConfigurationSectionProps {
  token: string
  endpoint: EndpointDetail
  isFormDirty: boolean
  onImported: (endpoint: EndpointDetail) => void
}

export function ConfigurationSection({ token, endpoint, isFormDirty, onImported }: ConfigurationSectionProps) {
  const { toast } = useToast()
  const importer = useConfigImport({ token, endpoint, onImported })
  const exporter = useAsyncAction(() => exportConfig(token))
  const isBusy = exporter.pending || importer.isApplying
  const exportError =
    exporter.error === null
      ? null
      : exporter.error instanceof ExportError
        ? exporter.error.message
        : t('set.config.export.error')
  const { phase } = importer

  async function handleExport() {
    const result = await exporter.run()
    if (result.ok) toast(t('set.config.toast.exported'))
  }

  return (
    <SettingsSection id="configuration" title={t('set.config.title')} description={t('set.config.helper')}>
      {exportError && (
        <InlineAlert variant="danger" role="alert">
          {exportError}
        </InlineAlert>
      )}
      {importer.fileError && (
        <InlineAlert
          variant="danger"
          role="alert"
          action={
            <label htmlFor={IMPORT_INPUT_ID} className={FILE_LABEL_CLASSES}>
              {t('set.config.import.chooseAnother')}
            </label>
          }
        >
          {importer.fileError}
        </InlineAlert>
      )}

      <div className="flex flex-wrap gap-2">
        <Button variant="outline" onClick={() => void handleExport()} loading={exporter.pending} disabled={isBusy}>
          {exporter.pending ? t('set.config.export.busy') : t('set.config.export')}
        </Button>
        {/* The sr-only input must precede its label for peer-focus-visible. */}
        <input
          id={IMPORT_INPUT_ID}
          type="file"
          accept="application/json,.json"
          className="peer sr-only"
          disabled={isBusy}
          onChange={importer.handleFileChange}
          aria-label={t('set.config.import.aria')}
        />
        <label
          htmlFor={IMPORT_INPUT_ID}
          className={cn(
            FILE_LABEL_CLASSES,
            'peer-focus-visible:outline peer-focus-visible:outline-2 peer-focus-visible:outline-offset-2 peer-focus-visible:outline-focus',
            'peer-disabled:cursor-not-allowed peer-disabled:bg-surface-subtle peer-disabled:text-text-tertiary',
          )}
        >
          {t('set.config.import')}
        </label>
      </div>
      <div className="space-y-1 text-caption text-text-tertiary">
        {isFormDirty && <p>{t('set.config.export.dirty')}</p>}
        <p>{t('set.config.import.helper')}</p>
        <p>{t('set.config.import.fileHint')}</p>
      </div>

      {phase.kind === 'applying' && (
        <div className="space-y-1.5">
          <p className="text-body-sm text-text-secondary">{phase.label}</p>
          <Progress value={phase.value} max={phase.max} label={phase.label} />
          <p className="text-caption text-text-tertiary">{t('set.config.import.dontClose')}</p>
        </div>
      )}
      {phase.kind === 'report' && (
        <InlineAlert
          variant={phase.variant === 'danger' ? 'danger' : 'info'}
          role={phase.variant === 'danger' ? 'alert' : 'status'}
          action={
            <div className="flex flex-wrap gap-2">
              {phase.addedRules > 0 && (
                <Button variant="outline" size="sm" asChild>
                  <Link to={`/d/${token}/rules`}>{t('set.config.import.viewRules')}</Link>
                </Button>
              )}
              <Button variant="ghost" size="sm" onClick={importer.reset}>
                {t('common.dismiss')}
              </Button>
            </div>
          }
        >
          {phase.message}
        </InlineAlert>
      )}

      <ConfirmDialog
        open={phase.kind === 'confirming'}
        onClose={importer.closeConfirm}
        title={t('set.config.confirm.title')}
        body={
          phase.kind === 'confirming' ? (
            <ImportConfirmBody
              bundle={phase.bundle}
              diff={phase.diff}
              existingRuleCount={phase.existingRuleCount}
              isFormDirty={isFormDirty}
            />
          ) : null
        }
        confirmLabel={t('set.config.confirm.confirm')}
        confirmVariant="primary"
        onConfirm={importer.confirm}
      />
    </SettingsSection>
  )
}
