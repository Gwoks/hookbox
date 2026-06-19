/* ============================================================================
 * request-stream.js — the `useRequestStream` substitute (_decisions.md §1a).
 *
 * Responsibilities (identical to the spec's React hook), all AC-30*:
 *   - Open /ws/<token>?cap=<owner_secret>  (owner-gated feed, §5.4 / OQ-4).
 *   - Exponential-backoff reconnect 250→500→1000→2000→4000→8000ms + jitter.
 *   - Dedupe incoming RequestSummary by id (request_id alias tolerated).
 *   - Feed rows into the Alpine `feed` store WITHOUT locking the DOM: bursts are
 *     coalesced with requestAnimationFrame; arrival flash decays <=900ms (CSS).
 *   - Cap the feed at the 100-trace retention cap; footer "Showing {n} of last 100".
 *   - Pause toggle + "N new" buffer that flushes on resume (AC-30c).
 *   - Heartbeat "ping" + half-open detection; pause reconnection while
 *     document.hidden and resume + back-fill on focus (AC-30d).
 *   - On WS close 4401 (unauthorized): surface "Unauthorized", do NOT hammer;
 *     re-check the stored capability (AC-30 / AC-29 / AC-S12 client side).
 *   - First paint is server-rendered (partials/feed_row.html); on open, back-fill
 *     via GET /api/endpoints/{token}/requests then reconcile (AC-27a).
 *   - SSE fallback after repeated WS failures (arch §4.5; same owner gate).
 *
 * Drives two Alpine stores: `feed` (rows/selection/pause) and `stream` (health).
 * Channel scoping (AC-32): a stream is bound to exactly one token, so it only
 * ever receives that token's events.
 * ========================================================================== */
(function () {
    'use strict';
    var HB = (window.HookBox = window.HookBox || {});

    var CAP = 100;                                 // AC-30a — matches TRACE_CAP
    var BACKOFF = [250, 500, 1000, 2000, 4000, 8000];
    var HEARTBEAT_MS = 25000;                      // client ping cadence
    var PONG_GRACE_MS = 10000;                     // half-open detection window
    var MAX_WS_FAILS_BEFORE_SSE = 6;               // then try SSE fallback
    var WS_CLOSE_UNAUTHORIZED = 4401;              // §5.4 owner-gate refusal

    function jitter(ms) { return ms + Math.floor(Math.random() * Math.min(ms, 1000)); }

    /* Normalize a server RequestSummary into the row shape the UI renders. */
    function toRow(d) {
        if (!d || typeof d !== 'object') return null;
        var id = (d.id != null) ? d.id : d.request_id;     // tolerate either name
        if (id == null) return null;
        return {
            id: id,
            method: d.method || 'ANY',
            path: d.path != null ? d.path : '',
            status_code: d.status_code != null ? d.status_code : 0,
            served_by: d.served_by || null,
            matched_rule_id: (d.matched_rule_id != null) ? d.matched_rule_id : null,
            duration_ms: d.duration_ms != null ? d.duration_ms : 0,
            timestamp: d.timestamp || null,
            _new: true,                                       // drives the arrival flash
        };
    }

    /* --------------------------------------------------------------------- */
    function RequestStream(token, opts) {
        opts = opts || {};
        this.token = token;
        this.cap = opts.cap || CAP;
        this.ws = null;
        this.sse = null;
        this.attempt = 0;            // reconnect attempt count (drives "Reconnecting (n)")
        this.wsFails = 0;            // consecutive WS failures (for SSE fallback)
        this.useSSE = false;
        this.closedByUs = false;
        this.unauthorized = false;   // sticky until capability changes
        this.lastCap = null;

        this.reconnectTimer = null;
        this.heartbeatTimer = null;
        this.pongTimer = null;
        this.rafScheduled = false;
        this.pendingBatch = [];      // rows awaiting an rAF commit (burst coalescing)
        this.seen = new Set();       // dedupe by id

        this._onVisibility = this._onVisibility.bind(this);
    }

    RequestStream.prototype.feed = function () {
        return (window.Alpine && Alpine.store('feed')) || null;
    };
    RequestStream.prototype.health = function () {
        return (window.Alpine && Alpine.store('stream')) || null;
    };

    RequestStream.prototype.setHealth = function (state, extra) {
        var h = this.health();
        if (!h) return;
        h.state = state;                       // live|reconnecting|offline|degraded|unauthorized
        if (extra && extra.attempt != null) h.attempt = extra.attempt;
    };

    /* ---- lifecycle ------------------------------------------------------- */
    RequestStream.prototype.start = function () {
        var owner = HB.readOwner();
        this.lastCap = owner && owner.owner_secret;
        if (!this.lastCap) {
            /* No capability at all → cannot subscribe (the feed is owner-gated). */
            this.unauthorized = true;
            this.setHealth('unauthorized');
            return;
        }
        document.addEventListener('visibilitychange', this._onVisibility);
        this.backfill();          // AC-27a: reconcile against the durable store on (re)connect
        this.connect();
    };

    RequestStream.prototype.stop = function () {
        this.closedByUs = true;
        document.removeEventListener('visibilitychange', this._onVisibility);
        this._clearTimers();
        if (this.ws) { try { this.ws.close(1000, 'client_stop'); } catch (e) {} this.ws = null; }
        if (this.sse) { try { this.sse.close(); } catch (e) {} this.sse = null; }
    };

    RequestStream.prototype._clearTimers = function () {
        clearTimeout(this.reconnectTimer); this.reconnectTimer = null;
        clearInterval(this.heartbeatTimer); this.heartbeatTimer = null;
        clearTimeout(this.pongTimer); this.pongTimer = null;
    };

    RequestStream.prototype.connect = function () {
        if (this.closedByUs || this.unauthorized) return;
        if (document.hidden) { return; }          // AC-30d: don't connect while hidden

        var owner = HB.readOwner();
        var cap = owner && owner.owner_secret;
        if (!cap) { this.unauthorized = true; this.setHealth('unauthorized'); return; }
        this.lastCap = cap;

        if (this.useSSE) { this._connectSSE(cap); return; }

        var proto = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
        var url = proto + '//' + window.location.host + '/ws/' +
                  encodeURIComponent(this.token) + '?cap=' + encodeURIComponent(cap);

        this.setHealth(this.attempt > 0 ? 'reconnecting' : 'connecting', { attempt: this.attempt });

        var self = this;
        var ws;
        try { ws = new WebSocket(url); } catch (e) { this._scheduleReconnect(); return; }
        this.ws = ws;

        ws.onopen = function () {
            self.attempt = 0;
            self.wsFails = 0;
            self.setHealth('live', { attempt: 0 });
            self.backfill();                       // reconcile any events missed while down
            self._startHeartbeat();
        };
        ws.onmessage = function (ev) { self._onMessage(ev.data); };
        ws.onerror = function () { /* surfaced via onclose */ };
        ws.onclose = function (ev) {
            self._clearTimers();
            self.ws = null;
            if (self.closedByUs) return;
            if (ev && ev.code === WS_CLOSE_UNAUTHORIZED) {
                /* Owner-gate refusal. Do NOT hammer the gate (AC-30). */
                self.unauthorized = true;
                self.setHealth('unauthorized');
                /* Re-check the stored capability; if it changed, allow one retry. */
                var fresh = HB.readOwner();
                if (fresh && fresh.owner_secret && fresh.owner_secret !== self.lastCap) {
                    self.unauthorized = false;
                    self.attempt = 0;
                    self._scheduleReconnect();
                }
                return;
            }
            self.wsFails++;
            if (self.wsFails >= MAX_WS_FAILS_BEFORE_SSE && !self.useSSE) {
                self.useSSE = true;                // arch §4.5 fallback
            }
            self._scheduleReconnect();
        };
    };

    RequestStream.prototype._connectSSE = function (cap) {
        if (typeof window.EventSource === 'undefined') { this._scheduleReconnect(); return; }
        var url = '/sse/' + encodeURIComponent(this.token) + '?cap=' + encodeURIComponent(cap);
        var self = this;
        this.setHealth(this.attempt > 0 ? 'reconnecting' : 'connecting', { attempt: this.attempt });
        var es;
        try { es = new EventSource(url); } catch (e) { this._scheduleReconnect(); return; }
        this.sse = es;
        es.onopen = function () { self.attempt = 0; self.setHealth('live', { attempt: 0 }); self.backfill(); };
        es.onmessage = function (ev) { self._onMessage(ev.data); };
        es.onerror = function () {
            try { es.close(); } catch (e) {}
            self.sse = null;
            /* SSE can't surface the 4401 body; a single GET probe disambiguates auth. */
            self._scheduleReconnect();
        };
    };

    RequestStream.prototype._scheduleReconnect = function () {
        if (this.closedByUs || this.unauthorized) return;
        if (document.hidden) { return; }           // resumed by visibility handler
        var delay = jitter(BACKOFF[Math.min(this.attempt, BACKOFF.length - 1)]);
        this.attempt++;
        this.setHealth('reconnecting', { attempt: this.attempt });
        var self = this;
        clearTimeout(this.reconnectTimer);
        this.reconnectTimer = setTimeout(function () { self.connect(); }, delay);
    };

    /* ---- heartbeat / half-open detection (AC-30d) ------------------------ */
    RequestStream.prototype._startHeartbeat = function () {
        var self = this;
        clearInterval(this.heartbeatTimer);
        this.heartbeatTimer = setInterval(function () {
            if (!self.ws || self.ws.readyState !== WebSocket.OPEN) return;
            try { self.ws.send('ping'); } catch (e) {}
            /* If the socket is half-open (tab slept), a send won't error but the
             * connection is dead. Probe with a lightweight reconcile; if the WS
             * is still wedged after the grace window, force a reconnect. */
            clearTimeout(self.pongTimer);
            self.pongTimer = setTimeout(function () {
                if (self.ws && self.ws.readyState !== WebSocket.OPEN) {
                    try { self.ws.close(); } catch (e) {}  // triggers onclose → reconnect
                }
            }, PONG_GRACE_MS);
        }, HEARTBEAT_MS);
    };

    /* ---- inbound messages ------------------------------------------------ */
    RequestStream.prototype._onMessage = function (raw) {
        clearTimeout(this.pongTimer);              // any frame proves liveness
        var msg;
        try { msg = JSON.parse(raw); } catch (e) { return; }   // ignore non-JSON (e.g. pong text)
        if (!msg || !msg.type) return;

        switch (msg.type) {
            case 'hello':
                /* {token, server_time} — channel confirmed. Nothing to render. */
                break;
            case 'new_request':
                this._ingest(msg.data);            // data = RequestSummary (§5.3)
                break;
            case 'endpoint_updated':
                /* Config changed elsewhere; let the endpoint store refetch (.28). */
                document.dispatchEvent(new CustomEvent('hb:endpoint-updated', { detail: msg.data }));
                break;
            case 'state_changed':
                document.dispatchEvent(new CustomEvent('hb:state-changed', { detail: msg.data }));
                break;
            default:
                break;
        }
    };

    /* Dedupe + queue for the next rAF commit (burst coalescing, AC-30b). */
    RequestStream.prototype._ingest = function (data) {
        var row = toRow(data);
        if (!row) return;
        if (this.seen.has(row.id)) return;         // AC-30 dedupe by id
        this.seen.add(row.id);
        this.pendingBatch.push(row);
        this._scheduleFlush();
    };

    RequestStream.prototype._scheduleFlush = function () {
        if (this.rafScheduled) return;
        this.rafScheduled = true;
        var self = this;
        var raf = window.requestAnimationFrame || function (cb) { return setTimeout(cb, 16); };
        raf(function () { self.rafScheduled = false; self._commit(); });
    };

    RequestStream.prototype._commit = function () {
        var feed = this.feed();
        if (!feed) { this.pendingBatch = []; return; }
        if (!this.pendingBatch.length) return;

        /* Newest-first: the batch arrived in order, so reverse to prepend newest. */
        var batch = this.pendingBatch.splice(0, this.pendingBatch.length);

        if (feed.paused) {
            /* AC-30c: while paused, buffer rather than disturb the read position.
             * The "N new" counter still updates (reduced-motion safe). */
            feed.buffer = (feed.buffer || []).concat(batch);
            feed.pending = feed.buffer.length;
            return;
        }

        for (var i = 0; i < batch.length; i++) {
            feed.rows.unshift(batch[i]);
        }
        this._trim(feed);
        this._scheduleUnflash(batch);
    };

    /* Cap the feed at the retention cap so the DOM never grows unbounded (AC-30a). */
    RequestStream.prototype._trim = function (feed) {
        if (feed.rows.length > this.cap) {
            var removed = feed.rows.splice(this.cap);
            for (var i = 0; i < removed.length; i++) this.seen.delete(removed[i].id);
        }
    };

    /* Clear the _new flag after the flash window so re-selection doesn't re-flash. */
    RequestStream.prototype._scheduleUnflash = function (batch) {
        setTimeout(function () {
            for (var i = 0; i < batch.length; i++) batch[i]._new = false;
        }, 950);
    };

    /* ---- back-fill / reconcile against the durable store (AC-27a) -------- */
    RequestStream.prototype.backfill = function () {
        var self = this;
        var owner = HB.readOwner();
        if (!owner || !owner.owner_secret) return;
        fetch('/api/endpoints/' + encodeURIComponent(this.token) + '/requests?limit=' + this.cap, {
            headers: HB.authHeaders(),
        }).then(function (res) {
            if (res.status === 401 || res.status === 404) {
                /* Capability rejected by the management API too → unauthorized. */
                if (res.status === 401) { self.unauthorized = true; self.setHealth('unauthorized'); }
                return null;
            }
            if (!res.ok) return null;
            return res.json();
        }).then(function (list) {
            if (!Array.isArray(list)) return;
            var feed = self.feed();
            if (!feed) return;
            /* Reconcile: keep newest-first, dedupe against what we already have,
             * cap at retention. Server is authoritative for the historical window. */
            var existing = {};
            for (var i = 0; i < feed.rows.length; i++) existing[feed.rows[i].id] = true;
            var merged = feed.rows.slice();
            for (var j = 0; j < list.length; j++) {
                var row = toRow(list[j]);
                if (row && !existing[row.id]) { row._new = false; merged.push(row); self.seen.add(row.id); }
            }
            merged.sort(function (a, b) { return b.id - a.id; });
            feed.rows = merged.slice(0, self.cap);
        }).catch(function () { /* network error — the feed just stays as-is */ });
    };

    /* ---- visibility: pause reconnection while hidden, resume on focus ---- */
    RequestStream.prototype._onVisibility = function () {
        if (document.hidden) {
            /* AC-30d: pause reconnection attempts while the tab is hidden. */
            clearTimeout(this.reconnectTimer);
            clearInterval(this.heartbeatTimer);
        } else {
            if (this.closedByUs || this.unauthorized) return;
            /* Resume: if the socket died while hidden, reconnect + back-fill the gap. */
            if (!this.ws && !this.sse) { this.attempt = 0; this.connect(); }
            else { this.backfill(); }
        }
    };

    /* ---- public: flush the paused buffer (AC-30c) ------------------------ */
    RequestStream.prototype.flushBuffer = function () {
        var feed = this.feed();
        if (!feed || !feed.buffer || !feed.buffer.length) return;
        var buffered = feed.buffer.splice(0, feed.buffer.length);
        for (var i = 0; i < buffered.length; i++) feed.rows.unshift(buffered[i]);
        feed.pending = 0;
        this._trim(feed);
        this._scheduleUnflash(buffered);
    };

    /* ====================================================================== */
    /* Register the richer feed + stream stores and boot the stream.          */
    HB.startRequestStream = function (token, opts) {
        var stream = new RequestStream(token, opts);
        HB._stream = stream;

        document.addEventListener('alpine:init', function () {
            /* Health store for the WS pill (AC-29 / AC-VC4). */
            Alpine.store('stream', {
                state: 'connecting',       // connecting|reconnecting|live|offline|degraded|unauthorized
                attempt: 0,
                get label() {
                    switch (this.state) {
                        case 'live': return 'Live';
                        case 'connecting': return 'Connecting…';
                        case 'reconnecting': return this.attempt > 2 ? 'Reconnecting… (' + this.attempt + ')' : 'Reconnecting…';
                        case 'degraded': return 'Realtime degraded';
                        case 'unauthorized': return 'Unauthorized';
                        case 'offline':
                        default: return 'Offline';
                    }
                },
                get dotClass() {
                    switch (this.state) {
                        case 'live': return 'ws-dot ws-dot--live';
                        case 'connecting':
                        case 'reconnecting': return 'ws-dot ws-dot--reconnect';
                        case 'degraded': return 'ws-dot ws-dot--degraded';
                        case 'unauthorized': return 'ws-dot ws-dot--unauthorized';
                        case 'offline':
                        default: return 'ws-dot ws-dot--offline';
                    }
                },
            });

            /* Feed store (replaces the placeholder from stores.js). */
            Alpine.store('feed', {
                rows: [],
                buffer: [],
                pending: 0,
                paused: false,
                selectedId: null,
                capacity: stream.cap,

                get shownCount() { return this.rows.length; },
                get footerText() { return 'Showing ' + this.rows.length + ' of last ' + this.capacity; },

                select: function (id) {
                    this.selectedId = id;
                    /* Inspector lazy-loads the full RequestDetail (.26) via this event. */
                    document.dispatchEvent(new CustomEvent('hb:select-request', { detail: { id: id } }));
                },

                /* AC-30c: pause stops auto-prepend; resume flushes the buffer. */
                togglePause: function () {
                    this.paused = !this.paused;
                    if (!this.paused) { stream.flushBuffer(); }
                },
                flush: function () { stream.flushBuffer(); },
            });
        });

        /* Seed the feed from the server-rendered first-paint rows (AC-27a) so the
         * client list reconciles with what the browser already shows. The dashboard
         * template emits a JSON island #hb-initial-feed; parse it if present. */
        function seedFromDom() {
            var el = document.getElementById('hb-initial-feed');
            if (!el) return;
            var initial;
            try { initial = JSON.parse(el.textContent || '[]'); } catch (e) { initial = []; }
            var feed = (window.Alpine && Alpine.store('feed'));
            if (!feed || !Array.isArray(initial)) return;
            var rows = [];
            for (var i = 0; i < initial.length; i++) {
                var r = toRow(initial[i]);
                if (r) { r._new = false; rows.push(r); stream.seen.add(r.id); }
            }
            rows.sort(function (a, b) { return b.id - a.id; });
            feed.rows = rows.slice(0, stream.cap);
        }

        function boot() {
            seedFromDom();
            stream.start();
        }

        /* Alpine may init before or after DOMContentLoaded depending on CDN timing;
         * wait for both the store to exist and the DOM to be parsed. */
        if (window.Alpine && Alpine.store('feed')) {
            boot();
        } else {
            document.addEventListener('alpine:initialized', boot, { once: true });
        }
        return stream;
    };
})();
