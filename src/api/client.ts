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
import { apiPath, buildUrl, doFetch, parseJsonResponse, type CallOptions } from './http'
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

type Opts = CallOptions

export const api = {
  // #1 — no auth, no bounce on 401 (this route mints the cap).
  createSession(payload: SessionCreate, opts?: Opts) {
    return request(apiPath('session'), {
      method: 'POST',
      body: payload,
      noAuth: true,
      schema: sessionResponseSchema,
      signal: opts?.signal,
    })
  },
  // #2
  listEndpoints(opts?: Opts) {
    return request(apiPath('endpoints'), { schema: z.array(endpointSummarySchema), signal: opts?.signal })
  },
  // #3
  createEndpoint(payload: EndpointCreate, opts?: Opts) {
    return request(apiPath('endpoints'), {
      method: 'POST',
      body: payload,
      schema: endpointDetailSchema,
      signal: opts?.signal,
    })
  },
  // #4
  getEndpoint(token: string, opts?: Opts) {
    return request(apiPath('endpoints', token), { schema: endpointDetailSchema, signal: opts?.signal })
  },
  // #5
  patchEndpoint(token: string, payload: EndpointConfigPatch, opts?: Opts) {
    return request(apiPath('endpoints', token), {
      method: 'PATCH',
      body: payload,
      schema: endpointDetailSchema,
      signal: opts?.signal,
    })
  },
  // #6 — tombstone (410 on P1 thereafter); response is 200 Message.
  deleteEndpoint(token: string, opts?: Opts) {
    return request(apiPath('endpoints', token), { method: 'DELETE', schema: messageSchema, signal: opts?.signal })
  },
  // #7 — ORDER BY priority, id (server-guaranteed).
  listRules(token: string, opts?: Opts) {
    return request(apiPath('endpoints', token, 'rules'), {
      schema: z.array(mockRuleSchema),
      signal: opts?.signal,
    })
  },
  // #8
  createRule(token: string, payload: MockRuleCreate, opts?: Opts) {
    return request(apiPath('endpoints', token, 'rules'), {
      method: 'POST',
      body: payload,
      schema: mockRuleSchema,
      signal: opts?.signal,
    })
  },
  // #9
  getRule(token: string, id: number, opts?: Opts) {
    return request(apiPath('endpoints', token, 'rules', id), { schema: mockRuleSchema, signal: opts?.signal })
  },
  // #10
  patchRule(token: string, id: number, payload: MockRulePatch, opts?: Opts) {
    return request(apiPath('endpoints', token, 'rules', id), {
      method: 'PATCH',
      body: payload,
      schema: mockRuleSchema,
      signal: opts?.signal,
    })
  },
  // #11 — 204 no body.
  deleteRule(token: string, id: number, opts?: Opts) {
    return request<void>(apiPath('endpoints', token, 'rules', id), { method: 'DELETE', signal: opts?.signal })
  },
  // #12 — limit 1..200 (default 50), offset >= 0.
  listRequests(token: string, params?: { limit?: number; offset?: number }, opts?: Opts) {
    return request(apiPath('endpoints', token, 'requests'), {
      query: { limit: params?.limit, offset: params?.offset },
      schema: z.array(requestSummarySchema),
      signal: opts?.signal,
    })
  },
  // #13 — owner resolved via the trace's endpoint. `signal` lets a caller
  // abort an in-flight fetch (AC-48/AC-53 — the CSV export's shared
  // AbortController must actually cancel every in-flight
  // GET /api/requests/{id}, not just stop scheduling new ones).
  getRequest(id: number, opts?: Opts) {
    return request(apiPath('requests', id), { schema: requestDetailSchema, signal: opts?.signal })
  },
  // #14
  clearRequests(token: string, opts?: Opts) {
    return request(apiPath('endpoints', token, 'requests'), {
      method: 'DELETE',
      schema: messageSchema,
      signal: opts?.signal,
    })
  },
  // #15
  getState(token: string, opts?: Opts) {
    return request(apiPath('endpoints', token, 'state'), { schema: stateResponseSchema, signal: opts?.signal })
  },
  // #16
  clearState(token: string, opts?: Opts) {
    return request(apiPath('endpoints', token, 'state'), {
      method: 'DELETE',
      schema: messageSchema,
      signal: opts?.signal,
    })
  },
  // #17 — name ^[A-Za-z0-9_-]{1,64}$ else 422 invalid_collection.
  peekCollection(token: string, name: string, opts?: Opts) {
    return request(apiPath('endpoints', token, 'collections', name), {
      schema: collectionResponseSchema,
      signal: opts?.signal,
    })
  },
  // #18
  clearCollection(token: string, name: string, opts?: Opts) {
    return request(apiPath('endpoints', token, 'collections', name), {
      method: 'DELETE',
      schema: messageSchema,
      signal: opts?.signal,
    })
  },

  // ── F4 share links (operator-toolkit §5.1) — owner-authenticated ──
  // #19 — 201 ShareLinkCreated. `code`/`url` appear ONLY in this response.
  createShare(token: string, payload: ShareLinkCreate, opts?: Opts) {
    return request(apiPath('endpoints', token, 'shares'), {
      method: 'POST',
      body: payload,
      schema: shareLinkCreatedSchema,
      signal: opts?.signal,
    })
  },
  // #20 — ShareLink[], no code, no url, ever.
  listShares(token: string, opts?: Opts) {
    return request(apiPath('endpoints', token, 'shares'), {
      schema: z.array(shareLinkSchema),
      signal: opts?.signal,
    })
  },
  // #21 — by the non-secret integer `id`, NEVER the code (architecture D10).
  revokeShare(token: string, id: number, opts?: Opts) {
    return request<void>(apiPath('endpoints', token, 'shares', id), { method: 'DELETE', signal: opts?.signal })
  },

  // ── F4 public routes (operator-toolkit §5.2) — NO credential, ever ──
  // #22/#23 live in api/public-client.ts, NOT here: that module (and
  // everything it imports) must never reach session.ts, and this file
  // imports `session` above (AC-S13). The public /s/:code viewer imports
  // '@/api/public-client' directly, never '@/api'.
}
