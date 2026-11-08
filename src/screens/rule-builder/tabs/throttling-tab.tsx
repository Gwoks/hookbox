/** Throttling → the per-rule latency_ms / rate_limit_per_min / chaos_mode
 * overrides. Each can go back to "Inherit" (null = use the endpoint's
 * setting) — the old sliders could not return to null. */
import { t } from '@/lib/copy'
import { NumberSlider } from '@/components/ui/number-slider'
import { SelectField } from '@/components/ui/select'
import type { ChaosOverride } from '../draft'
import type { TabProps } from './types'

const LATENCY_MAX_MS = 10000
const RATE_LIMIT_MAX = 6000
const INHERIT = ''

const CHAOS_OPTIONS = [
  { value: INHERIT, label: t('rule.thr.chaosMode.inherit') },
  { value: 'error', label: t('rule.thr.chaosMode.error') },
  { value: 'dropout', label: t('rule.thr.chaosMode.dropout') },
]

export function ThrottlingTab({ draft, set }: TabProps) {
  return (
    <div className="space-y-6">
      <p className="text-body-sm text-text-secondary">{t('rule.thr.intro')}</p>
      <NumberSlider
        nullable
        label={t('rule.thr.latency.label')}
        unit={t('rule.thr.latency.unit')}
        helper={t('rule.thr.latency.helper')}
        min={0}
        max={LATENCY_MAX_MS}
        value={draft.latency_ms}
        onChange={(v) => set('latency_ms', v)}
      />
      <NumberSlider
        nullable
        label={t('rule.thr.rateLimit.label')}
        unit={t('rule.thr.rateLimit.unit')}
        helper={t('rule.thr.rateLimit.helper')}
        min={0}
        max={RATE_LIMIT_MAX}
        value={draft.rate_limit_per_min}
        onChange={(v) => set('rate_limit_per_min', v)}
      />
      <SelectField
        label={t('rule.thr.chaosMode.label')}
        options={CHAOS_OPTIONS}
        value={draft.chaos_mode ?? INHERIT}
        onChange={(e) => set('chaos_mode', e.target.value === INHERIT ? null : (e.target.value as ChaosOverride))}
      />
    </div>
  )
}
