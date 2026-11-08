/**
 * The endpoint settings form — pure. A SettingsForm is the editable shape of
 * the nine PATCH #5 fields; `toPatch` is the exact body Save sends (always
 * all nine, so a Save right after an import re-sends the imported values,
 * AC-89b); `isFormDirty` compares what Save would send.
 */
import type { ChaosMode, DefaultMode, EndpointConfigPatch, EndpointDetail } from '@/api/schemas'
import { t } from '@/lib/copy'

export interface SettingsForm {
  name: string
  target_url: string
  default_mode: DefaultMode
  auto_crud: boolean
  cors_enabled: boolean
  latency_ms: number
  rate_limit_per_min: number
  chaos_pct: number
  chaos_mode: ChaosMode
}

export interface SettingsErrors {
  target_url?: string
}

const PROXY_PROTOCOLS = ['http:', 'https:']

export function formFromEndpoint(endpoint: EndpointDetail): SettingsForm {
  return {
    name: endpoint.name ?? '',
    target_url: endpoint.target_url ?? '',
    default_mode: endpoint.default_mode,
    auto_crud: endpoint.auto_crud,
    cors_enabled: endpoint.cors_enabled,
    latency_ms: endpoint.latency_ms,
    rate_limit_per_min: endpoint.rate_limit_per_min,
    chaos_pct: endpoint.chaos_pct,
    chaos_mode: endpoint.chaos_mode,
  }
}

export function toPatch(form: SettingsForm): Required<EndpointConfigPatch> {
  return {
    name: form.name.trim() || null,
    target_url: form.target_url.trim() || null,
    default_mode: form.default_mode,
    auto_crud: form.auto_crud,
    cors_enabled: form.cors_enabled,
    latency_ms: form.latency_ms,
    rate_limit_per_min: form.rate_limit_per_min,
    chaos_pct: form.chaos_pct,
    chaos_mode: form.chaos_mode,
  }
}

export function isFormDirty(base: SettingsForm, draft: SettingsForm): boolean {
  const before = toPatch(base)
  const after = toPatch(draft)
  return (Object.keys(after) as (keyof typeof after)[]).some((key) => before[key] !== after[key])
}

function isProxyUrl(raw: string): boolean {
  try {
    const url = new URL(raw)
    return PROXY_PROTOCOLS.includes(url.protocol) && Boolean(url.host)
  } catch {
    return false
  }
}

export function validateSettings(form: SettingsForm): SettingsErrors {
  const target = form.target_url.trim()
  if (target && !isProxyUrl(target)) return { target_url: t('set.proxy.url.invalid') }
  return {}
}
