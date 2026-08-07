/** ShareDialog (design.md §3.6, operator-toolkit F4 owner side). Mint/list/
 * revoke read-only share links. Written for HASHED share codes
 * (architecture D9/D10/D11): the plaintext `code`/`url` exist ONLY in the
 * 201 response; the list carries `{ id, label, created_at, last_used_at }`
 * and revoke addresses a link by that non-secret integer `id`, never the
 * code. No per-row URL, no per-row copy, no Preview (AC-25). */
import { useEffect, useState } from 'react'
import { AlertTriangle, ExternalLink } from 'lucide-react'
import { ApiError, api, type ShareLink, type ShareLinkCreated } from '@/api'
import { t } from '@/lib/copy'
import { absolutize } from '@/lib/url'
import { relTime } from '@/lib/time'
import { Button } from '@/components/ui/button'
import { Field, Input } from '@/components/ui/input'
import { SkeletonLines } from '@/components/ui/skeleton'
import { Tooltip } from '@/components/ui/tooltip'
import { useToast } from '@/components/ui/toast'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
} from '@/components/ui/dialog'
import { InlineAlert } from './inline-alert'
import { CodeBlock } from './code-block'

// The documented default (backend/src/config.rs SHARE_MAX_PER_ENDPOINT). The
// API does not expose the configured value to the client, so this is a
// heuristic used only for the pre-emptive disable + copy — the server's own
// 422 is the actual enforcement (AC-27/AC-96).
const SHARE_MAX_PER_ENDPOINT = 10

const LOCAL_HOST_RE = /^(localhost|127\.0\.0\.1|::1|.*\.local)$/i
const RFC1918_RE = /^10\.|^172\.(1[6-9]|2\d|3[0-1])\.|^192\.168\./

function looksUnreachable(hostname: string): boolean {
  return LOCAL_HOST_RE.test(hostname) || RFC1918_RE.test(hostname)
}

// AC-81: a 404/410 on mint/list/revoke surfaces the shared endpoint-gone
// copy; otherwise the server's own detail (or a generic fallback).
function errorMessage(err: unknown, fallback: string): string {
  if (err instanceof ApiError) {
    if (err.status === 404 || err.status === 410) return t('common.error.endpointGone')
    return err.message || fallback
  }
  return fallback
}

type ListState =
  | { kind: 'loading' }
  | { kind: 'error'; message: string }
  | { kind: 'ready'; links: ShareLink[] }

export function ShareDialog({
  token,
  open,
  onOpenChange,
  onCountChange,
}: {
  token: string
  open: boolean
  onOpenChange: (open: boolean) => void
  onCountChange?: (count: number) => void
}) {
  const { toast } = useToast()
  const [list, setList] = useState<ListState>({ kind: 'loading' })
  const [label, setLabel] = useState('')
  const [creating, setCreating] = useState(false)
  const [createError, setCreateError] = useState<string | null>(null)
  const [justCreated, setJustCreated] = useState<ShareLinkCreated | null>(null)
  const [armedId, setArmedId] = useState<number | null>(null)
  const [revokingId, setRevokingId] = useState<number | null>(null)
  const [rowError, setRowError] = useState<{ id: number; message: string } | null>(null)
  const labelTooLong = label.trim().length > 80

  async function load() {
    setList({ kind: 'loading' })
    try {
      const links = await api.listShares(token)
      setList({ kind: 'ready', links })
      onCountChange?.(links.length)
    } catch (err) {
      setList({ kind: 'error', message: errorMessage(err, t('share.list.error.body')) })
    }
  }

  useEffect(() => {
    if (open) void load()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, token])

  function handleClose() {
    onOpenChange(false)
    setJustCreated(null)
    setCreateError(null)
    setArmedId(null)
    setRowError(null)
  }

  async function handleCreate() {
    if (labelTooLong) return
    setCreating(true)
    setCreateError(null)
    try {
      const result = await api.createShare(token, { label: label.trim() || null })
      setJustCreated(result)
      setLabel('')
      setList((prev) => {
        const row: ShareLink = {
          id: result.id,
          label: result.label,
          created_at: result.created_at,
          last_used_at: null,
        }
        return prev.kind === 'ready' ? { kind: 'ready', links: [row, ...prev.links] } : prev
      })
      onCountChange?.(list.kind === 'ready' ? list.links.length + 1 : 1)
      toast(t('share.toast.created'))
    } catch (err) {
      setCreateError(errorMessage(err, t('share.error.create')))
    } finally {
      setCreating(false)
    }
  }

  async function handleRevoke(id: number) {
    setRevokingId(id)
    setRowError(null)
    let alreadyRevoked = false
    try {
      await api.revokeShare(token, id)
    } catch (err) {
      if (err instanceof ApiError && err.status === 404) {
        alreadyRevoked = true // AC-95: the operator's intent is already satisfied
      } else {
        // AC-95: RESTORE the row (back to its normal display, not the armed
        // confirm) with a row-level error next to the still-live link.
        setRevokingId(null)
        setArmedId(null)
        const message = errorMessage(err, t('share.error.revoke'))
        setRowError({ id, message })
        toast(t('share.error.revoke'), 'danger')
        return
      }
    }
    setRevokingId(null)
    setArmedId(null)
    await load()
    toast(alreadyRevoked ? t('share.toast.revokedAlready') : t('share.toast.revoked'))
  }

  const atCap = list.kind === 'ready' && list.links.length >= SHARE_MAX_PER_ENDPOINT

  return (
    <Dialog open={open} onOpenChange={(o) => !o && handleClose()}>
      <DialogContent
        onEscapeKeyDown={(e) => {
          // AC-95: Esc while a row is armed cancels the ARM, not the dialog.
          if (armedId != null) {
            e.preventDefault()
            setArmedId(null)
          }
        }}
      >
        <DialogHeader>{t('share.title')}</DialogHeader>
        <div className="flex-1 overflow-y-auto px-6 py-5">
          <DialogDescription className="text-body text-text-secondary">
            {t('share.intro')}
          </DialogDescription>

          <InlineAlert variant="warning" role="status" title={t('share.warning.title')} className="mt-3">
            <p>{t('share.warning.body')}</p>
            <p className="mt-1.5">{t('share.warning.redaction')}</p>
          </InlineAlert>

          <div className="mt-4 space-y-3">
            <Field
              label={t('share.label.label')}
              helper={!labelTooLong ? t('share.label.helper') : undefined}
              error={labelTooLong ? t('share.label.tooLong') : null}
              render={(p) => (
                <Input
                  id={p.id}
                  aria-describedby={p.describedBy}
                  invalid={p.invalid}
                  placeholder={t('share.label.placeholder')}
                  value={label}
                  disabled={creating}
                  onChange={(e) => setLabel(e.target.value)}
                />
              )}
            />
            <div className="flex justify-end">
              <Button
                variant="primary"
                onClick={() => void handleCreate()}
                loading={creating}
                disabled={labelTooLong || atCap}
              >
                {creating ? t('share.creating') : t('share.create')}
              </Button>
            </div>
            {atCap && (
              <p className="text-caption text-text-tertiary">
                {t('share.limit.reached', { max: SHARE_MAX_PER_ENDPOINT })}
              </p>
            )}
            {createError && (
              <InlineAlert variant="danger" role="alert">
                {createError}
              </InlineAlert>
            )}
          </div>

          {justCreated && (
            <CreatedPanel result={justCreated} onDismiss={() => setJustCreated(null)} />
          )}

          <div className="mt-5 space-y-2">
            <div className="flex items-center justify-between">
              <h3 className="text-h4 text-text-primary">{t('share.list.title')}</h3>
              {list.kind === 'ready' && (
                <span className="tnum text-caption text-text-tertiary">
                  {t('share.list.count', { n: list.links.length, max: SHARE_MAX_PER_ENDPOINT })}
                </span>
              )}
            </div>
            <p className="text-caption text-text-tertiary">{t('share.list.hint')}</p>

            {list.kind === 'loading' && (
              <div aria-busy="true" className="p-3">
                <span className="sr-only">{t('share.list.loading.aria')}</span>
                <SkeletonLines lines={3} />
              </div>
            )}

            {list.kind === 'error' && (
              <InlineAlert
                variant="danger"
                role="alert"
                title={t('share.list.error.title')}
                action={
                  <Button variant="secondary" size="sm" onClick={() => void load()}>
                    {t('common.retry')}
                  </Button>
                }
              >
                {list.message}
              </InlineAlert>
            )}

            {list.kind === 'ready' && list.links.length === 0 && (
              <div className="rounded-md border border-border py-6 text-center">
                <h4 className="text-h4 text-text-primary">{t('share.list.empty.title')}</h4>
                <p className="mx-auto mt-1 max-w-xs text-body-sm text-text-tertiary">
                  {t('share.list.empty.body')}
                </p>
              </div>
            )}

            {list.kind === 'ready' && list.links.length > 0 && (
              <div className="divide-y divide-border overflow-hidden rounded-md border border-border">
                {list.links.map((link) =>
                  armedId === link.id ? (
                    <div key={link.id} className="-mx-3 bg-danger-bg px-3 py-2.5">
                      <p className="text-body-sm text-danger-fg">{t('share.row.revoke.confirm')}</p>
                      <p className="text-caption text-text-secondary">
                        {t('share.row.revoke.confirmHint')}
                      </p>
                      <div className="mt-2 flex justify-end gap-2">
                        <Button variant="ghost" size="sm" onClick={() => setArmedId(null)}>
                          {t('common.cancel')}
                        </Button>
                        <Button
                          variant="danger"
                          size="sm"
                          loading={revokingId === link.id}
                          onClick={() => void handleRevoke(link.id)}
                        >
                          {t('share.row.revoke.confirmAction')}
                        </Button>
                      </div>
                    </div>
                  ) : (
                    <div
                      key={link.id}
                      className="flex flex-wrap items-center justify-between gap-2 px-3 py-2.5"
                    >
                      <div className="min-w-0">
                        <p className="truncate text-body-sm text-text-primary">
                          {link.label || t('share.row.untitled')}
                        </p>
                        <p className="text-caption text-text-tertiary" title={link.created_at}>
                          {t('share.row.created', { when: relTime(link.created_at) })} ·{' '}
                          {link.last_used_at ? (
                            <Tooltip content={t('share.row.lastUsed.tooltip')}>
                              <span title={link.last_used_at}>
                                {t('share.row.lastUsed', { when: relTime(link.last_used_at) })}
                              </span>
                            </Tooltip>
                          ) : (
                            t('share.row.neverUsed')
                          )}
                        </p>
                        {rowError?.id === link.id && (
                          <p className="mt-1 text-body-sm text-danger-fg">{rowError.message}</p>
                        )}
                      </div>
                      <Button
                        variant="ghost"
                        size="sm"
                        className="text-danger-fg hover:bg-danger-bg hover:text-danger-fg"
                        aria-label={t('share.row.revoke.aria')}
                        onClick={() => setArmedId(link.id)}
                      >
                        {t('share.row.revoke')}
                      </Button>
                    </div>
                  ),
                )}
              </div>
            )}
          </div>
        </div>
        <DialogFooter>
          <Button variant="ghost" onClick={handleClose}>
            {t('share.done')}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}

function CreatedPanel({
  result,
  onDismiss,
}: {
  result: ShareLinkCreated
  onDismiss: () => void
}) {
  const url = absolutize(result.url)
  const unreachable = (() => {
    try {
      return looksUnreachable(new URL(url).hostname)
    } catch {
      return false
    }
  })()

  return (
    <div
      role="status"
      aria-live="polite"
      className="mt-4 space-y-2 rounded-md border border-border bg-accent-subtle-bg p-3"
    >
      <p className="text-overline uppercase tracking-wide text-text-tertiary">
        {t('share.created.title')}
      </p>
      <CodeBlock value={url} ariaLabel={t('share.created.title')} className="bg-surface" />
      <p className="flex items-start gap-1.5 text-caption text-warning-fg">
        <AlertTriangle className="mt-0.5 h-3.5 w-3.5 shrink-0" aria-hidden="true" />
        {t('share.created.onceHint')}
      </p>
      <p className="text-caption text-text-tertiary">{t('share.created.lostHint')}</p>
      {unreachable && (
        <p className="text-caption text-warning-fg">
          {t('share.created.localWarning', { origin: window.location.origin })}
        </p>
      )}
      <div className="flex justify-between">
        <Button variant="ghost" size="sm" asChild>
          <a
            href={url}
            target="_blank"
            rel="noopener noreferrer"
            aria-label={t('share.created.open.aria')}
          >
            <ExternalLink className="h-3.5 w-3.5" aria-hidden="true" />
            {t('share.created.open')}
          </a>
        </Button>
        <Button variant="ghost" size="sm" onClick={onDismiss}>
          {t('common.dismiss')}
        </Button>
      </div>
    </div>
  )
}
