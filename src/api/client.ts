/**
 * Typed API client over the 18 §5.2 management routes (PRD §5.1/§5.2/§5.3,
 * AC-1..5/60/S14/J7). The single typed boundary the SPA uses against the frozen
 * contract.
 *
 * - Attaches `Authorization: Bearer <owner_secret>` to ALL /api/** EXCEPT
 *   POST /api/session (§5.1). The cap comes from the memory/localStorage session
 *   store — NEVER a cookie (AC-S14), and requests are sent WITHOUT credentials so
 *   no ambient cookie auth is ever used.
 * - Parses the FLAT error envelope {error, detail} uniformly (AC-60).
 * - On a /api 401, bounces to / with the common.error.401 reason and clears the
 *   stale secret so a stale tab stops retrying until the stored secret changes
 *   (AC-J7); a 422 keeps the field-level detail; a 429 carries Retry-After.
 */
import { z } from 'zod'
import { ApiError } from './errors'
import { buildUrl, doFetch, parseJsonResponse } from './http'
import { session } from './session'
import {
  collectionResponseSchema,
  endpointDetailSchema,
  endpointSummarySchema,
  messageSchema,
  mockRuleSchema,
  requestDetailSchema,
  requestSummarySchema,
  sessionResponseSchema,
  shareLinkCreatedSchema,
  shareLinkSchema,
  stateResponseSchema,
  type EndpointConfigPatch,
  type EndpointCreate,
  type MockRuleCreate,
  type MockRulePatch,
  type SessionCreate,
  type ShareLinkCreate,
} from './schemas'

export { ApiError }

/** Set by the router so a /api 401 can redirect to the landing gate (§5.1). The
 * client stays framework-agnostic; the app registers a navigator at boot. */
let onUnauthorized: ((reason: string) => void) | null = null
export function setUnauthorizedHandler(fn: (reason: string) => void) {
  onUnauthorized = fn
}

interface RequestOpts<T> {
  method?: string
  body?: unknown
  /** POST /api/session is the only route that must NOT carry the Bearer cap. */
  noAuth?: boolean
  query?: Record<string, string | number | undefined>
  /** zod schema for the success body; its inferred type becomes the return type. */
  schema?: z.ZodType<T>
  /** Lets a caller abort an in-flight request (operator-toolkit AC-105f — the
   * public share viewer aborts its poll/detail fetches on unmount). */
  signal?: AbortSignal
}

async function request<T = void>(path: string, opts: RequestOpts<T> = {}): Promise<T> {
  const headers: Record<string, string> = {}
  if (opts.body !== undefined) headers['Content-Type'] = 'application/json'
  if (!opts.noAuth) {
    const secret = session.getSecret()
    if (secret) headers['Authorization'] = `Bearer ${secret}`
  }

  // No cookies — the cap is the only credential, attached as a header
  // (AC-S14). `doFetch`'s 'omit' guarantees no ambient cookie auth. An
  // aborted fetch is not a network failure — `doFetch` re-throws AbortError
  // as-is so the caller's own `signal.aborted` check can decide what to do.
  const res = await doFetch(buildUrl(path, opts.query), {
    method: opts.method ?? 'GET',
    headers,
    body: opts.body !== undefined ? JSON.stringify(opts.body) : undefined,
    signal: opts.signal,
  })

  // 401 on a /api route → the stored secret is stale/rotated. Clear it (so this
  // tab stops re-trying with the dead secret) and bounce to the gate (AC-J7).
  if (res.status === 401 && !opts.noAuth) {
    session.clear()
    onUnauthorized?.(
      'Your session ended — your secret was rotated somewhere else. Enter your email to continue.',
    )
    throw new ApiError('unauthorized', 'Unauthorized', 401)
  }

  return parseJsonResponse(res, opts.schema)
}

export const api = {
  // #1 — no auth, no bounce on 401 (this route mints the cap).
  createSession(payload: SessionCreate) {
    return request('/api/session', {
      method: 'POST',
      body: payload,
      noAuth: true,
      schema: sessionResponseSchema,
    })
  },
  // #2
  listEndpoints() {
    return request('/api/endpoints', { schema: z.array(endpointSummarySchema) })
  },
  // #3
  createEndpoint(payload: EndpointCreate) {
    return request('/api/endpoints', { method: 'POST', body: payload, schema: endpointDetailSchema })
  },
  // #4
  getEndpoint(token: string) {
    return request(`/api/endpoints/${encodeURIComponent(token)}`, { schema: endpointDetailSchema })
  },
  // #5
  patchEndpoint(token: string, payload: EndpointConfigPatch) {
    return request(`/api/endpoints/${encodeURIComponent(token)}`, {
      method: 'PATCH',
      body: payload,
      schema: endpointDetailSchema,
    })
  },
  // #6 — tombstone (410 on P1 thereafter); response is 200 Message.
  deleteEndpoint(token: string) {
    return request(`/api/endpoints/${encodeURIComponent(token)}`, {
      method: 'DELETE',
      schema: messageSchema,
    })
  },
  // #7 — ORDER BY priority, id (server-guaranteed).
  listRules(token: string) {
    return request(`/api/endpoints/${encodeURIComponent(token)}/rules`, {
      schema: z.array(mockRuleSchema),
    })
  },
  // #8
  createRule(token: string, payload: MockRuleCreate) {
    return request(`/api/endpoints/${encodeURIComponent(token)}/rules`, {
      method: 'POST',
      body: payload,
      schema: mockRuleSchema,
    })
  },
  // #9
  getRule(token: string, id: number) {
    return request(`/api/endpoints/${encodeURIComponent(token)}/rules/${id}`, {
      schema: mockRuleSchema,
    })
  },
  // #10
  patchRule(token: string, id: number, payload: MockRulePatch) {
    return request(`/api/endpoints/${encodeURIComponent(token)}/rules/${id}`, {
      method: 'PATCH',
      body: payload,
      schema: mockRuleSchema,
    })
  },
  // #11 — 204 no body.
  deleteRule(token: string, id: number) {
    return request<void>(`/api/endpoints/${encodeURIComponent(token)}/rules/${id}`, {
      method: 'DELETE',
    })
  },
  // #12 — limit 1..200 (default 50), offset >= 0.
  listRequests(token: string, params?: { limit?: number; offset?: number }) {
    return request(`/api/endpoints/${encodeURIComponent(token)}/requests`, {
      query: { limit: params?.limit, offset: params?.offset },
      schema: z.array(requestSummarySchema),
    })
  },
  // #13 — owner resolved via the trace's endpoint. `signal` lets a caller
  // abort an in-flight fetch (AC-48/AC-53 — the CSV export's shared
  // AbortController must actually cancel every in-flight
  // GET /api/requests/{id}, not just stop scheduling new ones).
  getRequest(id: number, opts?: { signal?: AbortSignal }) {
    return request(`/api/requests/${id}`, { schema: requestDetailSchema, signal: opts?.signal })
  },
  // #14
  clearRequests(token: string) {
    return request(`/api/endpoints/${encodeURIComponent(token)}/requests`, {
      method: 'DELETE',
      schema: messageSchema,
    })
  },
  // #15
  getState(token: string) {
    return request(`/api/endpoints/${encodeURIComponent(token)}/state`, {
      schema: stateResponseSchema,
    })
  },
  // #16
  clearState(token: string) {
    return request(`/api/endpoints/${encodeURIComponent(token)}/state`, {
      method: 'DELETE',
      schema: messageSchema,
    })
  },
  // #17 — name ^[A-Za-z0-9_-]{1,64}$ else 422 invalid_collection.
  peekCollection(token: string, name: string) {
    return request(`/api/endpoints/${encodeURIComponent(token)}/collections/${encodeURIComponent(name)}`, {
      schema: collectionResponseSchema,
    })
  },
  // #18
  clearCollection(token: string, name: string) {
    return request(`/api/endpoints/${encodeURIComponent(token)}/collections/${encodeURIComponent(name)}`, {
      method: 'DELETE',
      schema: messageSchema,
    })
  },

  // ── F4 share links (operator-toolkit §5.1) — owner-authenticated ──
  // #19 — 201 ShareLinkCreated. `code`/`url` appear ONLY in this response.
  createShare(token: string, payload: ShareLinkCreate) {
    return request(`/api/endpoints/${encodeURIComponent(token)}/shares`, {
      method: 'POST',
      body: payload,
      schema: shareLinkCreatedSchema,
    })
  },
  // #20 — ShareLink[], no code, no url, ever.
  listShares(token: string) {
    return request(`/api/endpoints/${encodeURIComponent(token)}/shares`, {
      schema: z.array(shareLinkSchema),
    })
  },
  // #21 — by the non-secret integer `id`, NEVER the code (architecture D10).
  revokeShare(token: string, id: number) {
    return request<void>(`/api/endpoints/${encodeURIComponent(token)}/shares/${id}`, {
      method: 'DELETE',
    })
  },

  // ── F4 public routes (operator-toolkit §5.2) — NO credential, ever ──
  // #22/#23 live in api/public-client.ts, NOT here: that module (and
  // everything it imports) must never reach session.ts, and this file
  // imports `session` above (AC-S13). The public /s/:code viewer imports
  // '@/api/public-client' directly, never '@/api'.
}
