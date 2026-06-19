/* ============================================================================
 * util.js — shared client helpers for HookBox (XSS-safe rendering + maps).
 *
 * Centralizes the things multiple modules need:
 *   - escapeHtml / setText : safe text insertion (AC-S14/AC-S15 — never raw innerHTML
 *     on captured data; values go in as text nodes / escaped).
 *   - method / status / served-by class+label maps : MIRROR of partials/feed_row.html
 *     so a WS-pushed row looks identical to a server-rendered one.
 *   - relativeTime : "12s ago" style stamps (extends the old formatDate).
 *
 * Pure functions on window.HookBox; no Alpine dependency so request-stream.js,
 * the inspector (.26), and Jinja-parity all share one source of truth.
 * ========================================================================== */
(function () {
    'use strict';
    var HB = (window.HookBox = window.HookBox || {});

    /* ---- XSS-safe text (AC-S14 / AC-S15) --------------------------------- */
    HB.escapeHtml = function (s) {
        if (s == null) return '';
        return String(s)
            .replace(/&/g, '&amp;')
            .replace(/</g, '&lt;')
            .replace(/>/g, '&gt;')
            .replace(/"/g, '&quot;')
            .replace(/'/g, '&#39;');
    };

    /* Set an element's text via textContent (never innerHTML) — the canonical
     * safe sink. Returns the element for chaining. */
    HB.setText = function (el, value) {
        if (el) el.textContent = value == null ? '' : String(value);
        return el;
    };

    /* ---- Method / status / served-by maps (mirror feed_row.html) --------- */
    HB.METHOD_BADGE = {
        GET: 'm-get', POST: 'm-post', PUT: 'm-put', PATCH: 'm-patch',
        DELETE: 'm-delete', OPTIONS: 'm-options', HEAD: 'm-head',
    };
    HB.METHOD_RAIL = {
        GET: 'rail-get', POST: 'rail-post', PUT: 'rail-put', PATCH: 'rail-patch',
        DELETE: 'rail-delete', OPTIONS: 'rail-options', HEAD: 'rail-head',
    };
    HB.SERVED_CLASS = {
        rule: 'served-rule', crud: 'served-crud', mitm: 'served-mitm', tunnel: 'served-tunnel',
        default: 'served-default', cors: 'served-cors', chaos: 'served-chaos', ratelimit: 'served-ratelimit',
    };
    HB.SERVED_LABEL = {
        rule: 'Matched rule', crud: 'Auto-CRUD', mitm: 'Proxied', tunnel: 'Tunneled',
        default: 'Default', cors: 'CORS', chaos: 'Chaos', ratelimit: 'Rate-limited',
    };

    HB.methodKey = function (m) { return (m || 'ANY').toString().toUpperCase(); };
    HB.methodBadgeClass = function (m) { return HB.METHOD_BADGE[HB.methodKey(m)] || 'm-any'; };
    HB.methodRailClass = function (m) { return HB.METHOD_RAIL[HB.methodKey(m)] || 'rail-any'; };

    HB.statusClass = function (code) {
        var c = parseInt(code, 10) || 0;
        if (c >= 200 && c < 300) return 'status-2xx';
        if (c >= 300 && c < 400) return 'status-3xx';
        if (c >= 400 && c < 500) return 'status-4xx';
        if (c >= 500) return 'status-5xx';
        return 'status-2xx';
    };

    HB.servedClass = function (s) { return HB.SERVED_CLASS[s] || 'served-default'; };
    HB.servedLabel = function (s) { return HB.SERVED_LABEL[s] || (s || ''); };

    /* ---- Body classification + JSON tree flattening (inspector, .26) ------
     * The inspector renders captured bodies CLIENT-SIDE from RequestDetail JSON
     * (arch §5). To keep rendering XSS-safe (AC-S14/AC-S15) we never build markup
     * from captured strings: instead we PARSE the body into a model and the
     * template binds every key/value with x-text (text nodes only). For JSON we
     * flatten the tree into an ordered, depth-tagged node list that a single
     * Alpine x-for can render with full collapse support and zero recursion in
     * markup (so `</script><script>` in any value is inert — it is a text node).
     */

    /* Heuristic content classification for a captured body + its content-type.
     * Returns one of: 'json' | 'xml' | 'binary' | 'empty' | 'text'. */
    HB.classifyBody = function (body, contentType) {
        if (body == null || body === '') return 'empty';
        var ct = (contentType || '').toLowerCase();
        /* Backend truncates/marks binary + oversized bodies; honor an explicit marker. */
        if (typeof body === 'string' && /^\s*\[(binary|truncated|binary, truncated)\b/i.test(body)) {
            return 'binary';
        }
        if (ct.indexOf('json') !== -1) {
            return HB.tryParseJson(body) !== undefined ? 'json' : 'text';
        }
        if (ct.indexOf('xml') !== -1 || ct.indexOf('html') !== -1) return 'xml';
        /* No/ambiguous content-type: sniff. A leading { or [ that parses is JSON. */
        if (typeof body === 'string') {
            var t = body.trim();
            if ((t.charAt(0) === '{' || t.charAt(0) === '[') && HB.tryParseJson(body) !== undefined) {
                return 'json';
            }
            if (t.charAt(0) === '<') return 'xml';
            /* Detect non-printable / control-heavy payloads as binary. */
            if (HB.looksBinary(t)) return 'binary';
        }
        return 'text';
    };

    /* Parse JSON; return the parsed value, or `undefined` if it is not JSON. */
    HB.tryParseJson = function (s) {
        if (typeof s !== 'string') return undefined;
        try {
            var v = JSON.parse(s);
            /* Only treat objects/arrays/scalars JSON.parse accepts as a "tree". */
            return v;
        } catch (e) {
            return undefined;
        }
    };

    /* Cheap binary sniff: a high ratio of control chars (excluding \t\r\n). */
    HB.looksBinary = function (s) {
        if (typeof s !== 'string' || !s.length) return false;
        var ctrl = 0, n = Math.min(s.length, 512);
        for (var i = 0; i < n; i++) {
            var c = s.charCodeAt(i);
            if (c === 9 || c === 10 || c === 13) continue;
            if (c < 32 || c === 127) ctrl++;
        }
        return ctrl / n > 0.1;
    };

    /* Flatten a parsed JSON value into an ordered node list for the tree view.
     * Each node: { id, depth, key, kind, preview, valueText, open, hasChildren,
     *              collapsible, childCount }.
     * kind ∈ 'object'|'array'|'string'|'number'|'boolean'|'null'.
     * `key` is null for the root and for array elements (those use an index label).
     * We cap total nodes so a pathological payload can't lock the DOM (AC-31). */
    HB.flattenJson = function (root, maxNodes) {
        var cap = maxNodes || 5000;
        var out = [];
        var seq = 0;

        function kindOf(v) {
            if (v === null) return 'null';
            if (Array.isArray(v)) return 'array';
            var t = typeof v;
            if (t === 'object') return 'object';
            if (t === 'number') return 'number';
            if (t === 'boolean') return 'boolean';
            return 'string';
        }

        function scalarText(v, kind) {
            if (kind === 'null') return 'null';
            if (kind === 'string') return v;            /* raw string; rendered via x-text */
            return String(v);
        }

        function walk(value, key, indexLabel, depth) {
            if (out.length >= cap) return;
            var kind = kindOf(value);
            var node = {
                id: 'n' + (seq++),
                depth: depth,
                key: key,                                 /* object key (string) or null */
                indexLabel: indexLabel,                   /* "[0]" for array items, else null */
                kind: kind,
                open: depth < 2,                          /* auto-expand first 2 levels */
                hasChildren: false,
                childCount: 0,
                preview: '',
                valueText: '',
            };
            if (kind === 'object' || kind === 'array') {
                var keys = kind === 'array' ? value : Object.keys(value);
                var count = kind === 'array' ? value.length : keys.length;
                node.hasChildren = count > 0;
                node.childCount = count;
                node.preview = kind === 'array' ? ('[' + count + ']') : ('{' + count + '}');
                out.push(node);
                if (kind === 'array') {
                    for (var i = 0; i < value.length; i++) {
                        walk(value[i], null, '[' + i + ']', depth + 1);
                    }
                } else {
                    for (var k = 0; k < keys.length; k++) {
                        walk(value[keys[k]], keys[k], null, depth + 1);
                    }
                }
            } else {
                node.valueText = scalarText(value, kind);
                out.push(node);
            }
        }

        walk(root, null, null, 0);
        if (out.length >= cap) {
            out.push({ id: 'n_truncated', depth: 0, key: null, indexLabel: null,
                       kind: 'note', open: false, hasChildren: false, childCount: 0,
                       preview: '', valueText: '… tree truncated (too large to render fully)' });
        }
        return out;
    };

    /* CSS tint class for a scalar node kind (VC-10). */
    HB.treeValueClass = function (kind) {
        switch (kind) {
            case 'string': return 'v-string';
            case 'number': return 'v-number';
            case 'boolean': return 'v-bool';
            case 'null': return 'v-null';
            default: return '';
        }
    };

    /* Pretty-print a captured body for the raw <pre> fallback (JSON re-indented
     * when parseable; otherwise returned as-is). Output is rendered via x-text. */
    HB.prettyBody = function (body, contentType) {
        var parsed = HB.tryParseJson(body);
        if (parsed !== undefined && parsed !== null && typeof parsed === 'object') {
            try { return JSON.stringify(parsed, null, 2); } catch (e) { /* fall through */ }
        }
        return body == null ? '' : String(body);
    };

    /* ---- Relative time --------------------------------------------------- */
    HB.relativeTime = function (tsInput) {
        if (!tsInput) return '';
        var ts = tsInput;
        /* §5 stamps are ISO-8601 UTC; ensure naive strings parse as UTC. */
        if (typeof ts === 'string' && !/[zZ]|[+-]\d\d:?\d\d$/.test(ts)) {
            ts = ts.replace(' ', 'T') + 'Z';
        }
        var d = new Date(ts);
        if (isNaN(d.getTime())) return String(tsInput);
        var diff = (Date.now() - d.getTime()) / 1000;
        if (diff < 0) diff = 0;
        if (diff < 5) return 'just now';
        if (diff < 60) return Math.floor(diff) + 's ago';
        if (diff < 3600) return Math.floor(diff / 60) + 'm ago';
        if (diff < 86400) return Math.floor(diff / 3600) + 'h ago';
        return d.toLocaleString();
    };
})();
