/**
 * Typed client over the 2 §5.2 PUBLIC routes (PRD §5.2, AC-22/23/42/S13) — the
 * ONLY module the public /s/:code viewer may import for network access.
 *
 * Deliberately separate from api/client.ts: that file imports session.ts to
 * attach the owner's `Authorization` header, and AC-S13 requires the
 * viewer's module graph to never reach session.ts (defence-in-depth — this
 * page renders attacker-supplied text with no CSP behind it). Every request
 * here carries no credential, ever (no header, no cookie — `doFetch` sends
 * `credentials: 'omit'`), so there is nothing for this module to attach and
 * no reason for it to depend on the session store at all.
 */
import type { ZodType } from 'zod'
import { buildUrl, doFetch, parseJsonResponse } from './http'
import {
  publicRequestDetailSchema,
  publicShareFeedSchema,
  type PublicRequestDetail,
  type PublicRequestSummary,
  type PublicShareFeed,
} from './schemas'

export { ApiError } from './errors'
export type { PublicRequestDetail, PublicRequestSummary, PublicShareFeed } from './schemas'

interface PublicRequestOpts<T> {
  query?: Record<string, string | number | undefined>
  schema?: ZodType<T>
  signal?: AbortSignal
}

async function publicRequest<T = void>(path: string, opts: PublicRequestOpts<T> = {}): Promise<T> {
  const res = await doFetch(buildUrl(path, opts.query), {
    method: 'GET',
    signal: opts.signal,
  })
  return parseJsonResponse(res, opts.schema)
}

export const api = {
  // #22 — a public 401 can never fire (there's no Bearer to reject), but this
  // also guarantees the request never carries the owner's Authorization
  // header (AC-42/AC-S13): this module has no header to attach in the first
  // place.
  getSharedRequests(
    code: string,
    params?: { limit?: number; offset?: number; signal?: AbortSignal },
  ): Promise<PublicShareFeed> {
    return publicRequest(`/api/share/${encodeURIComponent(code)}/requests`, {
      query: { limit: params?.limit, offset: params?.offset },
      schema: publicShareFeedSchema,
      signal: params?.signal,
    })
  },
  // #23
  getSharedRequest(
    code: string,
    id: number,
    opts?: { signal?: AbortSignal },
  ): Promise<PublicRequestDetail> {
    return publicRequest(`/api/share/${encodeURIComponent(code)}/requests/${id}`, {
      signal: opts?.signal,
      schema: publicRequestDetailSchema,
    })
  },
}
