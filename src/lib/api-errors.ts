/** Shared error → user-message mapping (operator-toolkit AC-81). Every
 * mutating control can race a delete happening in another tab, and each
 * should surface the same `common.error.endpointGone` copy rather than a
 * generic failure or the server's raw `detail` (which names a token that no
 * longer resolves to anything). Route catches through these helpers so the
 * mapping stays in one place.
 *
 * Imports ONLY `@/api/errors` (dependency-free), never the `@/api` barrel, so
 * viewer-safe components may use it without reaching the owner client. */
import { ApiError } from '@/api/errors'
import { t } from './copy'

/** A 404/410 from an endpoint-scoped route: the endpoint was deleted. */
export function isEndpointGone(err: unknown): err is ApiError {
  return err instanceof ApiError && (err.status === 404 || err.status === 410)
}

export interface ErrorMessageOptions {
  /** Map a 404/410 to the shared endpoint-gone copy instead of its detail. */
  mapGone?: boolean
}

/** The message to show for a caught error: the server's `detail` when it is
 * an `ApiError` carrying one, otherwise `fallback`. */
export function errorMessage(
  err: unknown,
  fallback: string,
  { mapGone = false }: ErrorMessageOptions = {},
): string {
  if (mapGone && isEndpointGone(err)) return t('common.error.endpointGone')
  if (err instanceof ApiError && err.message) return err.message
  return fallback
}

/** Rethrows `err` unchanged, EXCEPT a 404/410 `ApiError`, whose `detail` is
 * replaced with the shared endpoint-gone copy. Always throws — never returns
 * — so callers can write `catch (err) { remapEndpointGone(err) }`. */
export function remapEndpointGone(err: unknown): never {
  if (isEndpointGone(err)) {
    throw new ApiError(err.code, t('common.error.endpointGone'), err.status)
  }
  throw err
}
