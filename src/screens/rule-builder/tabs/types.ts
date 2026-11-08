import type { Draft, DraftErrors } from '../draft'

export type DraftSetter = <K extends keyof Draft>(key: K, value: Draft[K]) => void

export interface TabProps {
  draft: Draft
  set: DraftSetter
  errors: DraftErrors
}
