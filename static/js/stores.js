/* ============================================================================
 * stores.js — Alpine.js stores for HookBox.
 *
 * Single source of truth for client state (arch §5 file plan, _decisions.md §1a:
 * the React `useRequestStream` hook becomes an Alpine store + request-stream.js).
 *
 * Stores registered here:
 *   - owner      : identity / capability (this file — AC-3 / AC-3a / §5.1)
 *   - feed       : live request feed rows + selection (filled by request-stream.js / .25)
 *   - inspector  : lazy-loaded RequestDetail per tab (filled by .26)
 *   - endpoint   : current EndpointDetail + config (filled by .28)
 *   - rules      : rule list + edit state (filled by .27)
 *
 * Low-level helpers (HookBox.readOwner / writeOwner / authHeaders / …) are
 * defined inline in base.html so plain-JS (non-Alpine) code can use identity
 * before Alpine boots. This file layers the reactive Alpine stores on top.
 * ========================================================================== */
(function () {
    'use strict';

    /* Defensive: base.html always defines window.HookBox first, but guard in
     * case stores.js is ever loaded standalone (e.g. a test harness). */
    var HB = (window.HookBox = window.HookBox || {});
    if (typeof HB.readOwner !== 'function') {
        HB.STORAGE_KEY = HB.STORAGE_KEY || 'hookbox_owner';
        HB.readOwner = function () {
            try { return JSON.parse(window.localStorage.getItem(HB.STORAGE_KEY) || 'null'); }
            catch (e) { return null; }
        };
        HB.writeOwner = function (o) {
            try { window.localStorage.setItem(HB.STORAGE_KEY, JSON.stringify(o)); return true; }
            catch (e) { return false; }
        };
        HB.clearOwner = function () {
            try { window.localStorage.removeItem(HB.STORAGE_KEY); } catch (e) { /* noop */ }
        };
        HB.authHeaders = function () {
            var o = HB.readOwner();
            return o && o.owner_secret ? { 'Authorization': 'Bearer ' + o.owner_secret } : {};
        };
        HB.storageAvailable = (function () {
            try { window.localStorage.setItem('__hb_probe__', '1'); window.localStorage.removeItem('__hb_probe__'); return true; }
            catch (e) { return false; }
        })();
    }

    document.addEventListener('alpine:init', function () {
        /* ---- owner: identity + capability (AC-3 / AC-3a / §5.1) ----------- */
        Alpine.store('owner', {
            data: HB.readOwner(),
            storageAvailable: HB.storageAvailable,

            get isLoggedIn() { return !!(this.data && this.data.owner_secret); },
            get email() { return (this.data && this.data.email) || ''; },
            get token() { return (this.data && this.data.token) || ''; },
            get secret() { return (this.data && this.data.owner_secret) || ''; },
            get mockUrl() { return (this.data && this.data.mock_url) || ''; },

            /* Persist a freshly-issued session (from POST /api/session).
             * We keep only the fields §5.1 says belong in localStorage. */
            set: function (owner) {
                var slim = {
                    owner_id: owner.owner_id,
                    owner_secret: owner.owner_secret,
                    token: owner.token,
                    mock_url: owner.mock_url,
                    email: owner.email || (this.data && this.data.email) || undefined,
                };
                this.data = slim;
                HB.writeOwner(slim);
            },

            /* Update only the active token (used by the endpoint switcher, AC-3b)
             * without losing the capability. */
            setToken: function (token, mockUrl) {
                if (!this.data) return;
                this.data = Object.assign({}, this.data, {
                    token: token,
                    mock_url: mockUrl != null ? mockUrl : this.data.mock_url,
                });
                HB.writeOwner(this.data);
            },

            logout: function () {
                this.data = null;
                HB.clearOwner();
                window.location.href = '/';
            },
        });

        /* ---- inspector: lazy-loaded RequestDetail per tab (AC-31/31a/31b/31c)
         * Listens for hb:select-request (fired by the feed store on row click),
         * lazy-fetches GET /api/requests/{id} (§5.2 #13), and exposes the parsed
         * RequestDetail (§5.3) to partials/inspector.html. All captured data is
         * rendered by the template via x-text (text nodes only) — this store never
         * builds markup from captured strings, satisfying AC-S14/AC-S15.
         *
         * Detail availability under fire-and-forget (AC-31a): because traces are
         * written fire-and-forget, a just-clicked row's detail may not be in SQLite
         * yet → GET returns 404. We treat that as a transient "detail pending"
         * state (with bounded auto-retries + a manual Retry), NOT a hard error. */
        Alpine.store('inspector', {
            requestId: null,        // currently-selected request id
            detail: null,           // RequestDetail (§5.3) once loaded
            tab: 'headers',         // headers | query | body | response | trace
            status: 'idle',         // idle | loading | ready | pending | error | unauthorized
            error: '',
            _retries: 0,
            _retryTimer: null,
            _reqSeq: 0,             // guards against out-of-order responses

            MAX_AUTO_RETRIES: 6,    // ~ exponential up to a few seconds (AC-31a)

            get isLoading() { return this.status === 'loading'; },
            get isPending() { return this.status === 'pending'; },
            get isReady() { return this.status === 'ready' && !!this.detail; },
            get isError() { return this.status === 'error'; },
            get isUnauthorized() { return this.status === 'unauthorized'; },

            /* ----- parsed body views (computed, never cached as markup) ----- */
            get reqContentType() { return headerLookup(this.detail && this.detail.request_headers, 'content-type'); },
            get resContentType() {
                var d = this.detail; if (!d) return '';
                /* Prefer the rule/response content-type header; fall back to the response_headers map. */
                return headerLookup(d.response_headers, 'content-type');
            },
            get reqBodyKind() { return HB.classifyBody(this.detail && this.detail.request_body, this.reqContentType); },
            get resBodyKind() { return HB.classifyBody(this.detail && this.detail.response_body, this.resContentType); },
            get reqBodyTree() {
                if (this.reqBodyKind !== 'json') return [];
                var p = HB.tryParseJson(this.detail.request_body);
                return p === undefined ? [] : HB.flattenJson(p);
            },
            get resBodyTree() {
                if (this.resBodyKind !== 'json') return [];
                var p = HB.tryParseJson(this.detail.response_body);
                return p === undefined ? [] : HB.flattenJson(p);
            },
            get reqBodyRaw() { return HB.prettyBody(this.detail && this.detail.request_body, this.reqContentType); },
            get resBodyRaw() { return HB.prettyBody(this.detail && this.detail.response_body, this.resContentType); },

            /* The frozen X-HookBox-* identifying headers (§5.5, AC-31b), surfaced
             * from the captured response_headers (case-insensitive lookup). */
            get hbEndpoint() { return headerLookup(this.detail && this.detail.response_headers, 'x-hookbox-endpoint'); },
            get hbServedBy() { return headerLookup(this.detail && this.detail.response_headers, 'x-hookbox-served-by'); },
            get hbRuleId() { return headerLookup(this.detail && this.detail.response_headers, 'x-hookbox-rule-id'); },

            get servedLabel() { return HB.servedLabel(this.detail && this.detail.served_by); },
            get servedClass() { return HB.servedClass(this.detail && this.detail.served_by); },
            get methodBadge() { return HB.methodBadgeClass(this.detail && this.detail.method); },
            get statusClass() { return HB.statusClass(this.detail && this.detail.status_code); },

            /* KV lists for Headers / Query Params tabs (ordered, [k,v] pairs). */
            get requestHeaderPairs() { return toPairs(this.detail && this.detail.request_headers); },
            get responseHeaderPairs() { return toPairs(this.detail && this.detail.response_headers); },
            get queryPairs() { return toPairs(this.detail && this.detail.query_params); },

            /* Trace as a renderable step list (AC-31c): glyph + class per step kind,
             * plus a parsed before→after state diff when the detail encodes one. */
            get traceSteps() {
                var d = this.detail;
                if (!d || !Array.isArray(d.trace)) return [];
                return d.trace.map(function (t, i) {
                    return decorateTraceStep(t, i);
                });
            },
            get stateSnapshotPairs() { return toPairs(this.detail && this.detail.state_snapshot); },

            setTab: function (t) { this.tab = t; },

            select: function (id) {
                if (id == null) { this.clear(); return; }
                clearTimeout(this._retryTimer);
                this.requestId = id;
                this._retries = 0;
                this.detail = null;
                this.error = '';
                this.load();
            },

            clear: function () {
                clearTimeout(this._retryTimer);
                this.requestId = null;
                this.detail = null;
                this.status = 'idle';
                this.error = '';
                this._retries = 0;
            },

            retry: function () {
                if (this.requestId == null) return;
                this._retries = 0;
                this.load();
            },

            load: function () {
                var self = this;
                var id = this.requestId;
                if (id == null) return;
                var seq = ++this._reqSeq;
                this.status = 'loading';
                fetch('/api/requests/' + encodeURIComponent(id), {
                    headers: HB.authHeaders(),
                }).then(function (res) {
                    if (seq !== self._reqSeq || id !== self.requestId) return null; // superseded
                    if (res.status === 401) { self.status = 'unauthorized'; self.error = 'Not authorized.'; return null; }
                    if (res.status === 404) {
                        /* AC-31a: fire-and-forget write may not have landed yet.
                         * Auto-retry a bounded number of times before giving up,
                         * then offer a manual Retry — never a hard 404 to the user. */
                        if (self._retries < self.MAX_AUTO_RETRIES) {
                            self.status = 'pending';
                            var delay = Math.min(250 * Math.pow(2, self._retries), 4000);
                            self._retries++;
                            clearTimeout(self._retryTimer);
                            self._retryTimer = setTimeout(function () {
                                if (id === self.requestId) self.load();
                            }, delay);
                        } else {
                            self.status = 'pending';   // stays pending; manual Retry available
                        }
                        return null;
                    }
                    if (!res.ok) { self.status = 'error'; self.error = 'Could not load request detail.'; return null; }
                    return res.json();
                }).then(function (data) {
                    if (data == null) return;
                    if (seq !== self._reqSeq || id !== self.requestId) return; // superseded
                    self.detail = data;            // RequestDetail (§5.3)
                    self.status = 'ready';
                    self._retries = 0;
                }).catch(function () {
                    if (seq !== self._reqSeq || id !== self.requestId) return;
                    self.status = 'error';
                    self.error = 'Network error loading detail.';
                });
            },
        });

        /* The feed store fires hb:select-request on row click (request-stream.js).
         * Wire it to the inspector store here so the two stores stay decoupled. */
        document.addEventListener('hb:select-request', function (e) {
            var store = Alpine.store('inspector');
            if (store) store.select(e.detail && e.detail.id);
        });

        /* ---- placeholder stores (filled in by later FE waves) -------------
         * Registered empty here so x-data bindings referencing them never throw
         * if a partial loads before its wave ships. Each later issue REPLACES
         * the matching store body. The `feed`/`stream` stores are replaced by
         * request-stream.js when the dashboard boots. */
        if (!Alpine.store('feed')) {
            Alpine.store('feed', { rows: [], selectedId: null, paused: false, pending: 0, capacity: 100 });
        }
        /* ---- endpoint: current EndpointDetail + config + switcher (hookbox-wrd.28)
         * Serves the endpoint settings overlay and the multi-endpoint switcher.
         * - load(token)        : GET /api/endpoints/{token}  → EndpointDetail (§5.3)
         * - loadList()         : GET /api/endpoints          → EndpointSummary[] (AC-3b)
         * - save(patch)        : PATCH /api/endpoints/{token} (EndpointConfigPatch) AC-11/24/25
         * - createEndpoint()   : POST /api/endpoints         → new token (AC-3b)
         * - clearState()       : DELETE /api/endpoints/{token}/state (AC-10)
         * - clearHistory()     : DELETE /api/endpoints/{token}/requests (§5.2 #14)
         * Values are CLAMPED client-side before PATCH (AC-27c) — out-of-range is
         * never sent raw; the server also clamps (§5.3) so this is belt-and-braces. */
        Alpine.store('endpoint', {
            token: '',
            detail: null,           // EndpointDetail (§5.3)
            list: [],               // EndpointSummary[] for the switcher
            loading: false,
            listLoading: false,
            saving: false,
            error: '',
            saveError: '',
            creating: false,

            /* Clamp bounds — mirror §5.3 Field constraints exactly (AC-27c). */
            BOUNDS: {
                latency_ms: [0, 10000],
                rate_limit_per_min: [0, 100000],
                chaos_pct: [0, 100],
            },
            clampNum: function (key, raw) {
                var b = this.BOUNDS[key];
                var n = parseInt(raw, 10);
                if (isNaN(n)) n = b ? b[0] : 0;
                if (b) { if (n < b[0]) n = b[0]; if (n > b[1]) n = b[1]; }
                return n;
            },

            load: function (token) {
                var self = this;
                if (!token) return Promise.resolve(null);
                this.token = token;
                this.loading = true;
                this.error = '';
                return fetch('/api/endpoints/' + encodeURIComponent(token), {
                    headers: HB.authHeaders(),
                }).then(function (res) {
                    if (res.status === 401) { window.location.replace('/'); return null; }
                    if (res.status === 404) { self.error = 'Endpoint not found.'; self.loading = false; return null; }
                    if (!res.ok) { self.error = 'Could not load endpoint settings.'; self.loading = false; return null; }
                    return res.json();
                }).then(function (ep) {
                    if (ep) { self.detail = ep; }
                    self.loading = false;
                    return ep;
                }).catch(function () {
                    self.error = 'Network error loading endpoint.';
                    self.loading = false;
                    return null;
                });
            },

            loadList: function () {
                var self = this;
                this.listLoading = true;
                return fetch('/api/endpoints', { headers: HB.authHeaders() })
                    .then(function (res) {
                        if (res.status === 401) { window.location.replace('/'); return null; }
                        if (!res.ok) { self.listLoading = false; return null; }
                        return res.json();
                    }).then(function (list) {
                        if (Array.isArray(list)) self.list = list;
                        self.listLoading = false;
                        return self.list;
                    }).catch(function () { self.listLoading = false; return null; });
            },

            /* PATCH with a partial EndpointConfigPatch. Numeric fields are clamped
             * (AC-27c). Returns the updated EndpointDetail on success. */
            save: function (patch) {
                var self = this;
                if (!this.token) return Promise.resolve(null);
                /* Clamp any numeric fields present in the patch. */
                ['latency_ms', 'rate_limit_per_min', 'chaos_pct'].forEach(function (k) {
                    if (patch[k] !== undefined && patch[k] !== null) patch[k] = self.clampNum(k, patch[k]);
                });
                /* target_url: empty string clears MITM (per §5.3 "" or null clears). */
                if (patch.target_url !== undefined && patch.target_url !== null) {
                    patch.target_url = String(patch.target_url).trim();
                }
                this.saving = true;
                this.saveError = '';
                return fetch('/api/endpoints/' + encodeURIComponent(this.token), {
                    method: 'PATCH',
                    headers: Object.assign({ 'Content-Type': 'application/json' }, HB.authHeaders()),
                    body: JSON.stringify(patch),
                }).then(function (res) {
                    if (res.status === 401) { window.location.replace('/'); return null; }
                    if (res.status === 404) { self.saveError = 'Endpoint not found.'; self.saving = false; return null; }
                    if (res.status === 422) {
                        self.saving = false;
                        return res.json().then(function (b) {
                            self.saveError = (b && b.detail) ? ('Invalid: ' + (typeof b.detail === 'string' ? b.detail : 'check the values')) : 'Some values were rejected.';
                            return null;
                        }).catch(function () { self.saveError = 'Some values were rejected.'; return null; });
                    }
                    if (!res.ok) { self.saveError = 'Could not save settings.'; self.saving = false; return null; }
                    return res.json();
                }).then(function (ep) {
                    if (ep) {
                        self.detail = ep;
                        if (typeof showToast === 'function') showToast('Settings saved');
                    }
                    self.saving = false;
                    return ep;
                }).catch(function () {
                    self.saveError = 'Network error saving settings.';
                    self.saving = false;
                    return null;
                });
            },

            /* Convenience for the inline Auto-CRUD toggle (optimistic + revert). */
            setAutoCrud: function (value) {
                var self = this;
                var prev = this.detail ? this.detail.auto_crud : false;
                if (this.detail) this.detail.auto_crud = value;     // optimistic
                return this.save({ auto_crud: value }).then(function (ep) {
                    if (!ep && self.detail) {
                        self.detail.auto_crud = prev;               // revert on failure
                        if (typeof showToast === 'function') showToast('Could not update Auto-CRUD', true);
                    }
                    return ep;
                });
            },

            createEndpoint: function (name) {
                var self = this;
                this.creating = true;
                return fetch('/api/endpoints', {
                    method: 'POST',
                    headers: Object.assign({ 'Content-Type': 'application/json' }, HB.authHeaders()),
                    body: JSON.stringify({ name: name || null }),
                }).then(function (res) {
                    if (res.status === 401) { window.location.replace('/'); return null; }
                    if (!res.ok) { self.creating = false; if (typeof showToast === 'function') showToast('Could not create endpoint', true); return null; }
                    return res.json();
                }).then(function (ep) {        // EndpointDetail (§5.3)
                    self.creating = false;
                    if (ep && ep.token) {
                        /* Keep the active token in the owner store, then route (AC-3b). */
                        try { if (Alpine.store('owner')) Alpine.store('owner').setToken(ep.token, ep.mock_url); } catch (e) {}
                        window.location.assign('/d/' + encodeURIComponent(ep.token));
                    }
                    return ep;
                }).catch(function () { self.creating = false; if (typeof showToast === 'function') showToast('Network error creating endpoint', true); return null; });
            },

            /* Danger zone — clear per-endpoint Redis state (AC-10). */
            clearState: function () {
                var self = this;
                if (!this.token) return Promise.resolve(false);
                return fetch('/api/endpoints/' + encodeURIComponent(this.token) + '/state', {
                    method: 'DELETE', headers: HB.authHeaders(),
                }).then(function (res) {
                    if (res.status === 401) { window.location.replace('/'); return false; }
                    var ok = res.ok;
                    if (typeof showToast === 'function') showToast(ok ? 'State cleared' : 'Could not clear state', !ok);
                    return ok;
                }).catch(function () { if (typeof showToast === 'function') showToast('Network error clearing state', true); return false; });
            },

            /* Danger zone — clear captured trace history (§5.2 #14). */
            clearHistory: function () {
                var self = this;
                if (!this.token) return Promise.resolve(false);
                return fetch('/api/endpoints/' + encodeURIComponent(this.token) + '/requests', {
                    method: 'DELETE', headers: HB.authHeaders(),
                }).then(function (res) {
                    if (res.status === 401) { window.location.replace('/'); return false; }
                    var ok = res.ok;
                    if (ok) {
                        try { var f = Alpine.store('feed'); if (f) { f.rows = []; f.buffer = []; f.pending = 0; f.selectedId = null; } } catch (e) {}
                    }
                    if (typeof showToast === 'function') showToast(ok ? 'History cleared' : 'Could not clear history', !ok);
                    return ok;
                }).catch(function () { if (typeof showToast === 'function') showToast('Network error clearing history', true); return false; });
            },
        });

        /* ---- rules: list + CRUD for the Create/Edit Rule modal (hookbox-wrd.27)
         * - load(token)   : GET  /api/endpoints/{token}/rules → MockRule[] (ordered)
         * - create(token,payload) : POST  …/rules (MockRuleCreate) → MockRule (AC-33)
         * - update(token,id,patch): PATCH …/rules/{id} (MockRulePatch) → MockRule (AC-34)
         * - toggle(token,rule)    : optimistic enable/disable + revert+toast (AC-34)
         * - remove(token,id)      : DELETE …/rules/{id} (AC-34)
         * Payload/patch are built by HookBox.RuleBuilder.serialize (rule-builder.js). */
        Alpine.store('rules', {
            token: '',
            list: [],               // MockRule[] (§5.3), ordered by priority,id
            loading: false,
            error: '',

            load: function (token) {
                var self = this;
                if (!token) return Promise.resolve(null);
                this.token = token;
                this.loading = true;
                this.error = '';
                return fetch('/api/endpoints/' + encodeURIComponent(token) + '/rules', {
                    headers: HB.authHeaders(),
                }).then(function (res) {
                    if (res.status === 401) { window.location.replace('/'); return null; }
                    if (res.status === 404) { self.error = 'Endpoint not found.'; self.loading = false; return null; }
                    if (!res.ok) { self.error = 'Could not load rules.'; self.loading = false; return null; }
                    return res.json();
                }).then(function (list) {
                    if (Array.isArray(list)) self.list = list;
                    self.loading = false;
                    return self.list;
                }).catch(function () { self.error = 'Network error loading rules.'; self.loading = false; return null; });
            },

            /* POST a MockRuleCreate. Resolves {ok, rule, status, body}. The modal
             * surfaces server-side validation (422) by re-rendering its errors. */
            create: function (token, payload) {
                var self = this;
                return fetch('/api/endpoints/' + encodeURIComponent(token) + '/rules', {
                    method: 'POST',
                    headers: Object.assign({ 'Content-Type': 'application/json' }, HB.authHeaders()),
                    body: JSON.stringify(payload),
                }).then(function (res) {
                    if (res.status === 401) { window.location.replace('/'); return { ok: false, status: 401 }; }
                    return res.json().catch(function () { return null; }).then(function (body) {
                        if (res.status === 201) {
                            if (body) self.list.push(body);
                            self._sort();
                            if (typeof showToast === 'function') showToast('Rule created');
                            return { ok: true, rule: body, status: 201 };
                        }
                        return { ok: false, status: res.status, body: body };
                    });
                }).catch(function () { return { ok: false, status: 0 }; });
            },

            update: function (token, id, patch) {
                var self = this;
                return fetch('/api/endpoints/' + encodeURIComponent(token) + '/rules/' + encodeURIComponent(id), {
                    method: 'PATCH',
                    headers: Object.assign({ 'Content-Type': 'application/json' }, HB.authHeaders()),
                    body: JSON.stringify(patch),
                }).then(function (res) {
                    if (res.status === 401) { window.location.replace('/'); return { ok: false, status: 401 }; }
                    return res.json().catch(function () { return null; }).then(function (body) {
                        if (res.ok) {
                            if (body) {
                                var idx = self.list.findIndex(function (r) { return r.id === id; });
                                if (idx !== -1) self.list.splice(idx, 1, body);
                            }
                            self._sort();
                            if (typeof showToast === 'function') showToast('Rule saved');
                            return { ok: true, rule: body, status: res.status };
                        }
                        return { ok: false, status: res.status, body: body };
                    });
                }).catch(function () { return { ok: false, status: 0 }; });
            },

            /* AC-34: optimistic enable/disable toggle + revert + toast on failure.
             * Disable != delete — the rule body round-trips untouched. */
            toggle: function (token, rule) {
                var self = this;
                var idx = this.list.findIndex(function (r) { return r.id === rule.id; });
                if (idx === -1) return Promise.resolve(false);
                var prev = this.list[idx].enabled;
                var next = !prev;
                this.list[idx].enabled = next;            // optimistic
                return fetch('/api/endpoints/' + encodeURIComponent(token) + '/rules/' + encodeURIComponent(rule.id), {
                    method: 'PATCH',
                    headers: Object.assign({ 'Content-Type': 'application/json' }, HB.authHeaders()),
                    body: JSON.stringify({ enabled: next }),
                }).then(function (res) {
                    if (!res.ok) {
                        self.list[idx].enabled = prev;     // revert
                        if (typeof showToast === 'function') showToast('Could not update rule', true);
                        return false;
                    }
                    return res.json().then(function (body) {
                        if (body) self.list.splice(idx, 1, body);
                        return true;
                    }).catch(function () { return true; });
                }).catch(function () {
                    self.list[idx].enabled = prev;          // revert on network error
                    if (typeof showToast === 'function') showToast('Network error updating rule', true);
                    return false;
                });
            },

            remove: function (token, id) {
                var self = this;
                return fetch('/api/endpoints/' + encodeURIComponent(token) + '/rules/' + encodeURIComponent(id), {
                    method: 'DELETE', headers: HB.authHeaders(),
                }).then(function (res) {
                    if (res.status === 401) { window.location.replace('/'); return false; }
                    if (res.status === 204 || res.ok) {
                        self.list = self.list.filter(function (r) { return r.id !== id; });
                        if (typeof showToast === 'function') showToast('Rule deleted');
                        return true;
                    }
                    if (typeof showToast === 'function') showToast('Could not delete rule', true);
                    return false;
                }).catch(function () { if (typeof showToast === 'function') showToast('Network error deleting rule', true); return false; });
            },

            _sort: function () {
                this.list.sort(function (a, b) {
                    var pa = a.priority == null ? 100 : a.priority, pb = b.priority == null ? 100 : b.priority;
                    if (pa !== pb) return pa - pb;
                    return (a.id || 0) - (b.id || 0);
                });
            },
        });

        /* Keep the endpoint store fresh when config changes elsewhere (WS
         * endpoint_updated → hb:endpoint-updated; request-stream.js / .25). */
        document.addEventListener('hb:endpoint-updated', function (e) {
            var store = Alpine.store('endpoint');
            if (store && e.detail && e.detail.token && e.detail.token === store.token) {
                store.load(store.token);
            }
        });
    });

    /* ---- helpers (module-private) ---------------------------------------- */

    /* Stable [key, value] pairs from a dict; tolerates null/non-object. */
    function toPairs(obj) {
        if (!obj || typeof obj !== 'object') return [];
        return Object.keys(obj).map(function (k) { return { k: k, v: stringifyVal(obj[k]) }; });
    }
    function stringifyVal(v) {
        if (v == null) return '';
        if (typeof v === 'string') return v;
        try { return JSON.stringify(v); } catch (e) { return String(v); }
    }

    /* Case-insensitive header lookup over a {name: value} dict. */
    function headerLookup(dict, name) {
        if (!dict || typeof dict !== 'object') return '';
        var want = String(name).toLowerCase();
        var keys = Object.keys(dict);
        for (var i = 0; i < keys.length; i++) {
            if (keys[i].toLowerCase() === want) return stringifyVal(dict[keys[i]]);
        }
        return '';
    }

    /* Map a TraceEvent (§5.3: {step, detail}) to a renderable step.
     * Glyphs/classes mirror design.md §3.4: matched ●, skipped ○, state-write ◆,
     * CORS (muted), chaos ✕. We also parse a "key: before -> after" diff out of
     * the detail string when a state mutation encodes one, so the template can
     * render it as a colored diff rather than raw text. */
    function decorateTraceStep(t, i) {
        var step = (t && t.step != null) ? String(t.step) : '';
        var detail = (t && t.detail != null) ? String(t.detail) : '';
        var s = step.toLowerCase();
        var cls = 'trace-skipped', glyph = '○';
        if (/match/.test(s) && !/no.?match|skip|miss/.test(s)) { cls = 'trace-matched'; glyph = '●'; }
        else if (/state.?write|write|mutat/.test(s)) { cls = 'trace-statewrite'; glyph = '◆'; }
        else if (/chaos|error|fail|drop/.test(s)) { cls = 'trace-chaos'; glyph = '✕'; }
        else if (/cors|preflight/.test(s)) { cls = 'trace-cors'; glyph = '◇'; }
        else if (/skip|miss|no.?match/.test(s)) { cls = 'trace-skipped'; glyph = '○'; }
        else if (/template|forward|state.?read|read|crud|mitm|tunnel|rate|default/.test(s)) { cls = 'trace-cors'; glyph = '▸'; }

        /* before -> after diff (tolerant of "from X to Y" / "X => Y" wording). */
        var diff = null;
        var m = detail.match(/^(.*?):\s*(.*?)\s*(?:->|=>|→)\s*(.*)$/);
        if (m) diff = { label: m[1].trim(), before: m[2].trim(), after: m[3].trim() };

        return { idx: i, step: step, detail: detail, cls: cls, glyph: glyph, diff: diff };
    }
})();
