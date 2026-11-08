/** Simulated conditions: latency, rate limit and chaos (applied in that
 * documented order: rate limit → chaos → latency), on the shared
 * NumberSlider. */
import type { ChaosMode } from '@/api'
import { t } from '@/lib/copy'
import { NumberSlider } from '@/components/ui/number-slider'
import { SettingsSection } from '@/components/ui/section'
import { SelectField } from '@/components/ui/select'
import type { SettingsFormState } from '../use-settings-form'

const LATENCY_MAX_MS = 10000
const RATE_LIMIT_MAX = 6000
const CHAOS_MAX_PCT = 100

const CHAOS_MODES = [
  { value: 'error', label: t('set.cond.chaosMode.error') },
  { value: 'dropout', label: t('set.cond.chaosMode.dropout') },
]

export function ConditionsSection({ form }: { form: SettingsFormState }) {
  const { draft, set } = form
  return (
    <SettingsSection id="conditions" title={t('set.cond.title')} description={t('set.cond.helper')}>
      <NumberSlider
        label={t('set.cond.latency.label')}
        unit={t('set.cond.latency.unit')}
        helper={t('set.cond.latency.helper')}
        min={0}
        max={LATENCY_MAX_MS}
        value={draft.latency_ms}
        onChange={(v) => set('latency_ms', v)}
      />
      <NumberSlider
        label={t('set.cond.rateLimit.label')}
        unit={t('set.cond.rateLimit.unit')}
        helper={t('set.cond.rateLimit.helper')}
        min={0}
        max={RATE_LIMIT_MAX}
        value={draft.rate_limit_per_min}
        onChange={(v) => set('rate_limit_per_min', v)}
      />
      <NumberSlider
        label={t('set.cond.chaos.label')}
        unit={t('set.cond.chaos.unit')}
        helper={t('set.cond.chaos.helper')}
        min={0}
        max={CHAOS_MAX_PCT}
        value={draft.chaos_pct}
        onChange={(v) => set('chaos_pct', v)}
      />
      <SelectField
        label={t('set.cond.chaosMode.label')}
        options={CHAOS_MODES}
        value={draft.chaos_mode}
        onChange={(e) => set('chaos_mode', e.target.value as ChaosMode)}
      />
    </SettingsSection>
  )
}
