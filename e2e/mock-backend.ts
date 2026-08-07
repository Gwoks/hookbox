/**
 * In-spec §5 mock backend (PRD §5.2/§5.3/§5.4). Fulfills every /api/** route
 * the SPA calls with the FROZEN shapes + status codes + the flat {error,detail}
 * envelope, so the e2e suite runs without the Rust backend ("mock/real §5
 * backend", AC-55). A mutable in-memory store lets journeys mutate (create rule,
 * delete endpoint) and read back consistently.
 *
 * The live feed (WS/SSE) is stubbed to a no-op socket: the SPA's documented
 * at-most-once contract means it RECONCILES authoritative rows via GET
 * /api/endpoints/{token}/requests, which this mock serves — so feed content is
 * driven by mutating `store.requests` then letting the reconcile fetch run.
 */
import type { Page, Route } from "@playwright/test";

export interface MockOptions {
  /** Seed an authenticated session in localStorage before the app boots. */
  authed?: boolean;
  /** Override the endpoint detail returned by GET /api/endpoints/{token}. */
  endpoint?: Partial<EndpointDetail>;
  /** Seed the request feed list. */
  requests?: RequestSummary[];
  /** Seed the rules list. */
  rules?: MockRule[];
  /** Force GET /api/endpoints/{token} to fail with this status (404/410/500). */
  endpointStatus?: number;
  /** Force DELETE /api/endpoints/{token}/requests to fail with this status
   * (ConfirmDialog failure-path coverage, AC-83). */
  clearRequestsStatus?: number;
  /** Seed the owner's share-link list (F4). */
  shares?: ShareLinkStub[];
  /** The plaintext code that resolves to this endpoint via the PUBLIC
   * /api/share/:code/* routes. Defaults to a fixed test code — the mock does
   * not enforce the real shape gate (that's server-only). */
  shareCode?: string;
  /** Force the public share LIST route to 404 — unknown/revoked/tombstoned
   * are byte-identical per §5.2, so one flag covers all three. */
  shareUnavailable?: boolean;
  /** Force the public share routes to 429 with this Retry-After (seconds). */
  shareRateLimitRetryAfter?: number;
  /** Force the public share LIST route to a 503. */
  shareServerError?: boolean;
  /** Force the public share DETAIL route to this status (404 = "gone", not
   * terminal; anything >=500 = a real error). */
  shareDetailStatus?: number;
}

export interface ShareLinkStub {
  id: number;
  label: string | null;
  created_at: string;
  last_used_at: string | null;
}

export const DEFAULT_SHARE_CODE = "test-share-code-0123456789abcdef";

interface EndpointDetail {
  token: string;
  name: string | null;
  mock_url: string;
  path_url: string;
  auto_crud: boolean;
  target_url: string | null;
  default_mode: "mock_404" | "echo";
  latency_ms: number;
  rate_limit_per_min: number;
  chaos_pct: number;
  chaos_mode: "error" | "dropout";
  cors_enabled: boolean;
  tunnel_active: boolean;
  created_at: string;
  last_hit: string | null;
  request_count: number;
}
interface RequestSummary {
  id: number;
  token: string;
  method: string;
  path: string;
  status_code: number;
  served_by: string;
  matched_rule_id: number | null;
  duration_ms: number;
  overhead_ms: number;
  timestamp: string;
}
interface MockRule {
  id: number;
  token: string;
  name: string | null;
  priority: number;
  enabled: boolean;
  match: {
    method: string;
    path: string;
    headers: Record<string, string>;
    query: Record<string, string>;
    body_conditions: unknown[];
    state_requirements: unknown[];
  };
  response: {
    status_code: number;
    headers: Record<string, string>;
    body_template: string;
    content_type: string;
  };
  state_writes: unknown[];
  latency_ms: number | null;
  rate_limit_per_min: number | null;
  chaos_mode: "error" | "dropout" | null;
  webhook_action: { url: string; body_template: string } | null;
  created_at: string;
}

export const TOKEN = "ab12cd34";
export const SECRET = "owner_secret_testvalue_0123456789";
const NOW = "2026-06-21T12:00:00Z";

export function makeEndpoint(
  over: Partial<EndpointDetail> = {},
): EndpointDetail {
  return {
    token: TOKEN,
    name: "Checkout API",
    mock_url: `https://${TOKEN}.hookbox.test`,
    path_url: `http://localhost:8080/m/${TOKEN}`,
    auto_crud: true,
    target_url: null,
    default_mode: "mock_404",
    latency_ms: 0,
    rate_limit_per_min: 0,
    chaos_pct: 0,
    chaos_mode: "error",
    cors_enabled: false,
    tunnel_active: false,
    created_at: NOW,
    last_hit: null,
    request_count: 3,
    ...over,
  };
}

export function makeRequest(
  over: Partial<RequestSummary> = {},
): RequestSummary {
  return {
    id: 1,
    token: TOKEN,
    method: "GET",
    path: "/ping",
    status_code: 200,
    served_by: "default",
    matched_rule_id: null,
    duration_ms: 4,
    overhead_ms: 1,
    timestamp: NOW,
    ...over,
  };
}

function detail(r: RequestSummary) {
  return {
    ...r,
    // "authorization" mirrors the server's redact() (backend/src/helpers.rs),
    // which writes the literal "<redacted>" sentinel (AC-133).
    request_headers: {
      "user-agent": "curl/8.0",
      "x-trace": "abc",
      authorization: "<redacted>",
    },
    query_params: { q: "1" },
    request_body: r.method === "POST" ? '{"hello":"world"}' : null,
    response_headers: { "content-type": "application/json" },
    response_body: '{"ok":true}',
    trace: [
      { step: "plane", detail: "P1 mock catch-all" },
      { step: "match", detail: "no rule matched → default" },
    ],
    state_snapshot: { logged_in: "true" },
  };
}

export async function installMockBackend(page: Page, opts: MockOptions = {}) {
  const store = {
    endpoint: makeEndpoint(opts.endpoint),
    deleted: false,
    requests: opts.requests ?? [
      makeRequest({
        id: 3,
        method: "POST",
        path: "/orders",
        status_code: 201,
        served_by: "crud",
        timestamp: NOW,
      }),
      makeRequest({
        id: 2,
        method: "GET",
        path: "/orders/42",
        status_code: 200,
        served_by: "rule",
        matched_rule_id: 10,
      }),
      makeRequest({
        id: 1,
        method: "DELETE",
        path: "/orders/7",
        status_code: 500,
        served_by: "chaos",
      }),
    ],
    rules: opts.rules ?? [],
    nextRuleId: 100,
    shares: opts.shares ?? ([] as ShareLinkStub[]),
    nextShareId: 500,
    shareCode: opts.shareCode ?? DEFAULT_SHARE_CODE,
  };

  // Seed an authed session + light theme BEFORE the app boots.
  if (opts.authed) {
    await page.addInitScript(
      ([secret, token]) => {
        localStorage.setItem("hookbox-owner-secret", secret);
        localStorage.setItem("hookbox-owner-id", "owner_1");
        localStorage.setItem("hookbox-owner-email", "dev@example.com");
        void token;
      },
      [SECRET, TOKEN],
    );
  }

  // Stub WebSocket + EventSource to inert objects: the SPA reconciles via the
  // mocked GET routes (the §5.4 at-most-once contract), so no live frames needed
  // for the reconcile path. A test hook (__hookboxPushFrame) lets a spec push a
  // synthetic new_request onto the currently-open fake socket, for scenarios
  // (like the paused-buffer AC-76 case) that need a real arrival.
  await page.addInitScript(() => {
    class FakeWS {
      onopen: (() => void) | null = null;
      onclose: (() => void) | null = null;
      onerror: (() => void) | null = null;
      onmessage: ((ev: { data: string }) => void) | null = null;
      readyState = 1;
      constructor() {
        // @ts-expect-error test hook
        window.__hookboxFakeWs = this;
        setTimeout(() => this.onopen && this.onopen(), 0);
      }
      send() {}
      close() {}
      addEventListener() {}
      removeEventListener() {}
    }
    // @ts-expect-error override for tests
    window.WebSocket = FakeWS;
    // @ts-expect-error test hook: dispatch a raw {type,data} WS frame JSON
    // string onto the currently-open fake socket, as if the server sent it.
    window.__hookboxPushFrame = (raw: string) => {
      // @ts-expect-error test hook
      const ws = window.__hookboxFakeWs;
      if (ws && ws.onmessage) ws.onmessage({ data: raw });
    };
    class FakeES {
      onopen: (() => void) | null = null;
      onerror: (() => void) | null = null;
      constructor() {
        setTimeout(() => this.onopen && this.onopen(), 0);
      }
      addEventListener() {}
      close() {}
    }
    // @ts-expect-error override for tests
    window.EventSource = FakeES;
  });

  const json = (route: Route, status: number, body: unknown) =>
    route.fulfill({
      status,
      contentType: "application/json",
      body: JSON.stringify(body),
    });
  const err = (route: Route, status: number, code: string, det: string) =>
    route.fulfill({
      status,
      contentType: "application/json",
      body: JSON.stringify({ error: code, detail: det }),
    });

  await page.route("**/api/**", async (route) => {
    const req = route.request();
    const url = new URL(req.url());
    const path = url.pathname;
    const method = req.method();

    // #1 POST /api/session — mints a cap (no auth).
    if (path === "/api/session" && method === "POST") {
      const ep = { ...store.endpoint };
      return json(route, 200, {
        owner_id: "owner_1",
        owner_secret: SECRET,
        endpoints: [summary(ep)],
        primary: summary(ep),
      });
    }

    // #2 GET /api/endpoints — a deleted endpoint is tombstoned, so it drops
    // out of the list (mirrors the real backend; also what stops Landing's
    // auto-resume from redirecting straight back to the endpoint the caller
    // just deleted, journey.md J9).
    if (path === "/api/endpoints" && method === "GET") {
      return json(route, 200, store.deleted ? [] : [summary(store.endpoint)]);
    }

    // #4/#5/#6 GET/PATCH/DELETE /api/endpoints/{token}
    if (path === `/api/endpoints/${TOKEN}`) {
      if (method === "GET") {
        if (store.deleted || opts.endpointStatus === 410)
          return err(route, 410, "endpoint_gone", "Endpoint deleted.");
        if (opts.endpointStatus === 404)
          return err(route, 404, "unknown_endpoint", "No such endpoint.");
        if (opts.endpointStatus && opts.endpointStatus >= 500)
          return err(route, 500, "server_error", "Boom.");
        return json(route, 200, store.endpoint);
      }
      if (method === "PATCH") {
        const body = req.postDataJSON() ?? {};
        store.endpoint = { ...store.endpoint, ...body };
        return json(route, 200, store.endpoint);
      }
      if (method === "DELETE") {
        store.deleted = true;
        return json(route, 200, {
          message: "Endpoint deleted.",
          success: true,
        });
      }
    }

    // #7/#8 rules list + create
    if (path === `/api/endpoints/${TOKEN}/rules`) {
      if (method === "GET") {
        const sorted = [...store.rules].sort(
          (a, b) => a.priority - b.priority || a.id - b.id,
        );
        return json(route, 200, sorted);
      }
      if (method === "POST") {
        const body = req.postDataJSON() ?? {};
        const rule = makeRule(store.nextRuleId++, body);
        store.rules.push(rule);
        return json(route, 200, rule);
      }
    }

    // #10/#11 rule patch/delete
    const ruleMatch = path.match(
      new RegExp(`^/api/endpoints/${TOKEN}/rules/(\\d+)$`),
    );
    if (ruleMatch) {
      const id = Number(ruleMatch[1]);
      const idx = store.rules.findIndex((r) => r.id === id);
      if (method === "PATCH") {
        if (idx < 0) return err(route, 404, "unknown_rule", "No such rule.");
        store.rules[idx] = {
          ...store.rules[idx],
          ...(req.postDataJSON() ?? {}),
        };
        return json(route, 200, store.rules[idx]);
      }
      if (method === "DELETE") {
        if (idx >= 0) store.rules.splice(idx, 1);
        return route.fulfill({ status: 204, body: "" });
      }
    }

    // #12 list requests (newest-first, server DESC)
    if (path === `/api/endpoints/${TOKEN}/requests`) {
      if (method === "GET") {
        const sorted = [...store.requests].sort((a, b) => b.id - a.id);
        return json(route, 200, sorted);
      }
      if (method === "DELETE") {
        if (opts.clearRequestsStatus) {
          return err(
            route,
            opts.clearRequestsStatus,
            "server_error",
            "Simulated failure for the confirm-dialog test.",
          );
        }
        store.requests = [];
        return json(route, 200, { message: "History cleared.", success: true });
      }
    }

    // #13 GET /api/requests/{id}
    const reqMatch = path.match(/^\/api\/requests\/(\d+)$/);
    if (reqMatch && method === "GET") {
      const id = Number(reqMatch[1]);
      // id 999 = a freshly-streamed row whose trace isn't persisted yet (PENDING).
      if (id === 999)
        return err(route, 404, "unknown_request", "Not written yet.");
      const found =
        store.requests.find((r) => r.id === id) ?? makeRequest({ id });
      return json(route, 200, detail(found));
    }

    // #15/#16 state
    if (path === `/api/endpoints/${TOKEN}/state`) {
      if (method === "GET")
        return json(route, 200, { state: { logged_in: "true" } });
      if (method === "DELETE")
        return json(route, 200, { message: "State cleared.", success: true });
    }

    // #17/#18 collections
    const colMatch = path.match(
      new RegExp(`^/api/endpoints/${TOKEN}/collections/([^/]+)$`),
    );
    if (colMatch) {
      const name = decodeURIComponent(colMatch[1]);
      if (!/^[A-Za-z0-9_-]{1,64}$/.test(name))
        return err(route, 422, "invalid_collection", "Bad name.");
      if (method === "GET")
        return json(route, 200, { items: [{ id: "1", name: "Ada" }] });
      if (method === "DELETE")
        return json(route, 200, {
          message: "Collection cleared.",
          success: true,
        });
    }

    // #19/#20 POST/GET /api/endpoints/{token}/shares (F4 owner)
    if (path === `/api/endpoints/${TOKEN}/shares`) {
      if (method === "POST") {
        const body = req.postDataJSON() ?? {};
        const id = store.nextShareId++;
        const label = (body.label as string | null) ?? null;
        const row: ShareLinkStub = {
          id,
          label,
          created_at: NOW,
          last_used_at: null,
        };
        store.shares = [row, ...store.shares];
        return json(route, 201, {
          id,
          code: store.shareCode,
          url: `/s/${store.shareCode}`,
          label,
          created_at: NOW,
          last_used_at: null,
        });
      }
      if (method === "GET") {
        return json(route, 200, store.shares);
      }
    }

    // #21 DELETE /api/endpoints/{token}/shares/{id} — by non-secret integer
    // id, never the code.
    const shareRevokeMatch = path.match(
      new RegExp(`^/api/endpoints/${TOKEN}/shares/(\\d+)$`),
    );
    if (shareRevokeMatch && method === "DELETE") {
      const id = Number(shareRevokeMatch[1]);
      const idx = store.shares.findIndex((s) => s.id === id);
      if (idx < 0) {
        return err(route, 404, "not_found", "Share link not found.");
      }
      store.shares.splice(idx, 1);
      return route.fulfill({ status: 204, body: "" });
    }

    // #22/#23 PUBLIC GET /api/share/{code}/requests[/{id}] — no credential.
    const shareCodeMatch = path.match(
      /^\/api\/share\/([^/]+)\/requests(?:\/(\d+))?$/,
    );
    if (shareCodeMatch && method === "GET") {
      const code = decodeURIComponent(shareCodeMatch[1]);
      const reqId = shareCodeMatch[2] ? Number(shareCodeMatch[2]) : null;

      if (opts.shareRateLimitRetryAfter != null) {
        return route.fulfill({
          status: 429,
          contentType: "application/json",
          headers: { "Retry-After": String(opts.shareRateLimitRetryAfter) },
          body: JSON.stringify({
            error: "rate_limited",
            detail: "Too many requests.",
          }),
        });
      }
      // The ONE 404 body — unknown / revoked / tombstoned / unknown request
      // id / cross-endpoint id are all byte-identical (AC-36).
      const shareNotFound = () =>
        err(route, 404, "not_found", "This share link is not available.");

      if (reqId != null) {
        if (opts.shareDetailStatus === 404) return shareNotFound();
        if (opts.shareDetailStatus && opts.shareDetailStatus >= 500) {
          return err(route, opts.shareDetailStatus, "store_unavailable", "Boom.");
        }
        if (opts.shareUnavailable || code !== store.shareCode) return shareNotFound();
        const found = store.requests.find((r) => r.id === reqId);
        if (!found) return shareNotFound();
        return json(route, 200, publicDetail(found));
      }

      if (opts.shareServerError) {
        return err(route, 503, "store_unavailable", "Boom.");
      }
      if (opts.shareUnavailable || code !== store.shareCode) return shareNotFound();
      const sorted = [...store.requests].sort((a, b) => b.id - a.id);
      return json(route, 200, {
        endpoint: {
          name: store.endpoint.name,
          created_at: store.endpoint.created_at,
          request_count: store.endpoint.request_count,
        },
        requests: sorted.map(publicSummary),
      });
    }

    return err(route, 404, "not_found", "Unhandled mock route.");
  });

  return store;
}

/** Push a synthetic `new_request` WS frame onto the currently-open fake
 * socket (see the `__hookboxPushFrame` test hook above) — for scenarios that
 * need a real arrival rather than the initial reconcile GET, e.g. AC-76's
 * "paused with a buffered arrival" case. */
export async function pushNewRequest(
  page: Page,
  over: Partial<RequestSummary> = {},
) {
  const frame = JSON.stringify({
    type: "new_request",
    data: makeRequest(over),
  });
  await page.evaluate((raw) => {
    // @ts-expect-error test hook
    window.__hookboxPushFrame(raw);
  }, frame);
}

// §5.5.4/5.5.5 public projections — omit token/matched_rule_id/overhead_ms/
// trace/state_snapshot; the five detail fields are present-with-null keys.
function publicSummary(r: RequestSummary) {
  return {
    id: r.id,
    method: r.method,
    path: r.path,
    status_code: r.status_code,
    served_by: r.served_by,
    duration_ms: r.duration_ms,
    timestamp: r.timestamp,
  };
}

function publicDetail(r: RequestSummary) {
  const d = detail(r);
  return {
    ...publicSummary(r),
    request_headers: d.request_headers,
    query_params: d.query_params,
    request_body: d.request_body,
    response_headers: d.response_headers,
    response_body: d.response_body,
  };
}

function summary(e: EndpointDetail) {
  return {
    token: e.token,
    name: e.name,
    mock_url: e.mock_url,
    path_url: e.path_url,
    created_at: e.created_at,
    last_hit: e.last_hit,
    request_count: e.request_count,
  };
}

export function makeRule(id: number, body: Record<string, unknown>): MockRule {
  const match = (body.match as MockRule["match"]) ?? {};
  const response = (body.response as MockRule["response"]) ?? {};
  return {
    id,
    token: TOKEN,
    name: (body.name as string | null) ?? null,
    priority: (body.priority as number) ?? 100,
    enabled: (body.enabled as boolean) ?? true,
    match: {
      method: match.method ?? "ANY",
      path: match.path ?? "/*",
      headers: match.headers ?? {},
      query: match.query ?? {},
      body_conditions: match.body_conditions ?? [],
      state_requirements: match.state_requirements ?? [],
    },
    response: {
      status_code: response.status_code ?? 200,
      headers: response.headers ?? {},
      body_template: response.body_template ?? "",
      content_type: response.content_type ?? "application/json",
    },
    state_writes: (body.state_writes as unknown[]) ?? [],
    latency_ms: (body.latency_ms as number | null) ?? null,
    rate_limit_per_min: (body.rate_limit_per_min as number | null) ?? null,
    chaos_mode: (body.chaos_mode as "error" | "dropout" | null) ?? null,
    webhook_action: (body.webhook_action as MockRule["webhook_action"]) ?? null,
    created_at: NOW,
  };
}
