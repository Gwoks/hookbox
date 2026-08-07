/** Shared 404/410 → "endpoint gone mid-session" remapping (operator-toolkit
 * AC-81). The dashboard shell only handles a 404/410 on the INITIAL load
 * today; every new mutating control this batch adds (F1 Clear all, F3
 * export/import, F4 share mint/list/revoke, F5 export) can also race a
 * delete happening in another tab, and each should surface the same
 * `common.error.endpointGone` message rather than a generic failure or the
 * server's raw `detail` (which names a token that no longer resolves to
 * anything). Route every such catch through this helper so the mapping
 * stays in one place. */
import { ApiError } from '@/api'
import { t } from './copy'

/** Rethrows `err` unchanged, EXCEPT a 404/410 `ApiError`, whose `detail` is
 * replaced with the shared endpoint-gone copy. Always throws — never returns
 * — so callers can write `catch (err) { remapEndpointGone(err) }`. */
export function remapEndpointGone(err: unknown): never {
  if (err instanceof ApiError && (err.status === 404 || err.status === 410)) {
    throw new ApiError(err.code, t('common.error.endpointGone'), err.status)
  }
  throw err
}
