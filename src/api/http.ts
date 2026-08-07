/**
 * Session-agnostic HTTP core shared by the owner client (api/client.ts) and
 * the public share-viewer client (api/public-client.ts): URL building, the
 * fetch call + abort passthrough, the flat `{error, detail}` envelope
 * (AC-60), and zod response validation.
 *
 * Attaching the owner's `Authorization` header and the /api 401 session-
 * bounce are OWNER-ONLY concerns and stay in client.ts. This module — and
 * everything it imports — must never import session.ts (AC-S13): the public
 * viewer's module graph roots here via public-client.ts.
 */
import { z } from 'zod'
import { ApiError } from './errors'
import { errorEnvelopeSchema } from './schemas'

export function buildUrl(path: string, query?: Record<string, string | number | undefined>): string {
  if (!query) return path
  const sp = new URLSearchParams()
  for (const [k, v] of Object.entries(query)) {
    if (v !== undefined && v !== '') sp.set(k, String(v))
  }
  const qs = sp.toString()
  return qs ? `${path}?${qs}` : path
}

/** `fetch` wrapped so a network failure becomes an `ApiError('network', ...)`
 * while a deliberate `AbortError` still propagates as itself (callers check
 * `signal.aborted`). */
export async function doFetch(url: string, init: RequestInit): Promise<Response> {
  try {
    return await fetch(url, { credentials: 'omit', ...init })
  } catch (err) {
    if (err instanceof DOMException && err.name === 'AbortError') throw err
    throw new ApiError('network', 'Network error. Check your connection and try again.', 0)
  }
}

/** Parses a successful/failed `Response` into `T`: 204 → `undefined`, a
 * non-2xx → the flat error envelope (AC-60) as an `ApiError` (carrying
 * `Retry-After` when present), a non-JSON 2xx → `undefined`, otherwise the
 * body validated against `schema` (when given) or returned as-is. */
export async function parseJsonResponse<T>(res: Response, schema?: z.ZodType<T>): Promise<T> {
  if (res.status === 204) return undefined as T

  if (!res.ok) {
    let code = 'error'
    let detail = 'Something went wrong. Try again.'
    try {
      const parsed = errorEnvelopeSchema.safeParse(await res.json())
      if (parsed.success) {
        code = parsed.data.error
        if (parsed.data.detail) detail = parsed.data.detail
      }
    } catch {
      /* non-JSON error body */
    }
    const ra = res.headers.get('Retry-After')
    throw new ApiError(code, detail, res.status, ra ? Number(ra) : undefined)
  }

  const ct = res.headers.get('Content-Type') ?? ''
  if (!ct.includes('application/json')) return undefined as T
  const json = await res.json()
  if (schema) {
    const parsed = schema.safeParse(json)
    if (!parsed.success) {
      throw new ApiError('contract_mismatch', 'Unexpected response shape from the server.', res.status)
    }
    return parsed.data as T
  }
  return json as T
}
