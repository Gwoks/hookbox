/* ============================================================================
 * rule-builder.js — serialize + validate the multi-tab Create/Edit Rule modal
 * (hookbox-wrd.27; AC-33, AC-33a, AC-34, AC-VC14).
 *
 * The 5 tabs (Matching · Response · Templating · Actions · Throttling) share ONE
 * flat form model. This module is the pure seam between that flat model and the
 * frozen MockRuleCreate / MockRulePatch shapes (§5.3): it has no DOM and no
 * Alpine dependency, so it is unit-testable and the modal stays declarative.
 *
 *   HookBox.RuleBuilder.blankForm()         → a fresh flat form (defaults = §5.3)
 *   HookBox.RuleBuilder.fromRule(rule)      → flat form seeded from a MockRule
 *   HookBox.RuleBuilder.validate(form)      → { ok, errors:{field→msg}, tabErrors:{tab→bool}, count }
 *   HookBox.RuleBuilder.serialize(form)     → MockRuleCreate (object)
 *
 * Tab ownership of fields (drives the rail state dots, AC-VC14):
 *   matching  : method, path, matchHeaders[], matchQuery[], bodyConditions[], stateReqs[]
 *   response  : statusCode, contentType, responseHeadersText (JSON), bodyTemplate
 *   templating: (bodyTemplate shared with response; tag palette only — no own fields)
 *   actions   : stateWrites[], webhook (DISABLED sub-section, still serialized — AC-33a)
 *   throttling: latencyOverride, rateLimitOverride, priority, enabled, name
 * ========================================================================== */
(function () {
    'use strict';
    var HB = (window.HookBox = window.HookBox || {});

    var METHODS = ['ANY', 'GET', 'POST', 'PUT', 'PATCH', 'DELETE', 'HEAD', 'OPTIONS'];
    var BODY_OPS = ['eq', 'neq', 'contains', 'exists'];
    var STATE_OPS = ['eq', 'neq', 'exists', 'absent'];

    function clampInt(v, lo, hi, dflt) {
        var n = parseInt(v, 10);
        if (isNaN(n)) return dflt;
        if (n < lo) n = lo;
        if (n > hi) n = hi;
        return n;
    }

    /* Parse a JSON object from a textarea; returns {ok, value, error}. Empty = {}. */
    function parseJsonObject(text) {
        var t = (text == null ? '' : String(text)).trim();
        if (t === '') return { ok: true, value: {} };
        var v;
        try { v = JSON.parse(t); } catch (e) { return { ok: false, error: 'Invalid JSON: ' + e.message }; }
        if (v === null || typeof v !== 'object' || Array.isArray(v)) {
            return { ok: false, error: 'Expected a JSON object (e.g. {"X-Header": "value"}).' };
        }
        /* Coerce all values to strings — dict[str,str] per §5.3. */
        var out = {};
        Object.keys(v).forEach(function (k) { out[k] = v[k] == null ? '' : String(v[k]); });
        return { ok: true, value: out };
    }

    /* Does the body contain template tags? If so we skip strict JSON validation
     * (a templated JSON body like {"id": {{random 'uuid'}}} is not valid JSON yet
     * but is legal per §5.7). */
    function hasTemplateTags(s) { return /\{\{/.test(s || ''); }

    /* Pair rows [{k,v}] → dict[str,str], dropping blank keys. */
    function pairsToDict(pairs) {
        var out = {};
        (pairs || []).forEach(function (p) {
            var k = (p && p.k != null) ? String(p.k).trim() : '';
            if (k) out[k] = (p.v == null ? '' : String(p.v));
        });
        return out;
    }
    function dictToPairs(d) {
        if (!d || typeof d !== 'object') return [];
        return Object.keys(d).map(function (k) { return { k: k, v: d[k] == null ? '' : String(d[k]) }; });
    }

    var RuleBuilder = {
        METHODS: METHODS,
        BODY_OPS: BODY_OPS,
        STATE_OPS: STATE_OPS,

        blankForm: function () {
            return {
                name: '',
                priority: 100,
                enabled: true,
                // matching
                method: 'ANY',
                path: '/*',
                matchHeaders: [],        // [{k,v}]
                matchQuery: [],          // [{k,v}]
                bodyConditions: [],      // [{path, op, value}]
                stateReqs: [],           // [{key, op, value}]
                // response
                statusCode: 200,
                contentType: 'application/json',
                responseHeadersText: '',  // JSON object text
                bodyTemplate: '',
                // actions
                stateWrites: [],          // [{key, value}]
                webhookEnabled: false,    // DISABLED in UI (AC-33a) but serialized
                webhookUrl: '',
                webhookBodyTemplate: '',
                // throttling
                latencyOverride: '',      // '' = no override (null)
                rateLimitOverride: '',    // '' = no override (null)
            };
        },

        /* Seed a flat form from an existing MockRule (§5.3) for the edit flow. */
        fromRule: function (rule) {
            var f = this.blankForm();
            if (!rule) return f;
            f.name = rule.name || '';
            f.priority = (rule.priority != null) ? rule.priority : 100;
            f.enabled = rule.enabled !== false;
            var m = rule.match || {};
            f.method = m.method || 'ANY';
            f.path = m.path || '/*';
            f.matchHeaders = dictToPairs(m.headers);
            f.matchQuery = dictToPairs(m.query);
            f.bodyConditions = (m.body_conditions || []).map(function (c) {
                return { path: c.path || '', op: c.op || 'eq', value: c.value == null ? '' : c.value };
            });
            f.stateReqs = (m.state_requirements || []).map(function (s) {
                return { key: s.key || '', op: s.op || 'eq', value: s.value == null ? '' : s.value };
            });
            var r = rule.response || {};
            f.statusCode = (r.status_code != null) ? r.status_code : 200;
            f.contentType = r.content_type || 'application/json';
            f.responseHeadersText = (r.headers && Object.keys(r.headers).length)
                ? JSON.stringify(r.headers, null, 2) : '';
            f.bodyTemplate = r.body_template || '';
            f.stateWrites = (rule.state_writes || []).map(function (w) {
                return { key: w.key || '', value: w.value == null ? '' : w.value };
            });
            if (rule.webhook_action) {
                f.webhookEnabled = true;
                f.webhookUrl = rule.webhook_action.url || '';
                f.webhookBodyTemplate = rule.webhook_action.body_template || '';
            }
            f.latencyOverride = (rule.latency_ms != null) ? rule.latency_ms : '';
            f.rateLimitOverride = (rule.rate_limit_per_min != null) ? rule.rate_limit_per_min : '';
            return f;
        },

        /* Validate → per-field messages + which tabs own an error (AC-VC14). */
        validate: function (form) {
            var errors = {};
            var tabErrors = { matching: false, response: false, templating: false, actions: false, throttling: false };

            // --- matching: required method + path ---
            if (!form.method || METHODS.indexOf(form.method) === -1) {
                errors.method = 'Choose a method.'; tabErrors.matching = true;
            }
            if (!form.path || !String(form.path).trim()) {
                errors.path = 'Path is required (e.g. /users/:id or /books/*).'; tabErrors.matching = true;
            }
            // body conditions: path required when a row exists; value required unless op=exists
            (form.bodyConditions || []).forEach(function (c, i) {
                if (!c.path || !String(c.path).trim()) {
                    errors['bodyCondition.' + i] = 'JSONPath is required.'; tabErrors.matching = true;
                }
                if (c.op !== 'exists' && (c.value == null || c.value === '')) {
                    errors['bodyConditionVal.' + i] = 'Value required for this operator.'; tabErrors.matching = true;
                }
            });
            (form.stateReqs || []).forEach(function (s, i) {
                if (!s.key || !String(s.key).trim()) {
                    errors['stateReq.' + i] = 'State key is required.'; tabErrors.matching = true;
                } else if (!/^[A-Za-z0-9_-]{1,64}$/.test(s.key)) {
                    errors['stateReq.' + i] = 'Key must be A–Z, 0–9, _ or - (max 64).'; tabErrors.matching = true;
                }
                if ((s.op === 'eq' || s.op === 'neq') && (s.value == null || s.value === '')) {
                    errors['stateReqVal.' + i] = 'Value required for this operator.'; tabErrors.matching = true;
                }
            });

            // --- response: status 100-599; headers must be valid JSON object ---
            var sc = parseInt(form.statusCode, 10);
            if (isNaN(sc) || sc < 100 || sc > 599) {
                errors.statusCode = 'Status code must be 100–599.'; tabErrors.response = true;
            }
            var hp = parseJsonObject(form.responseHeadersText);
            if (!hp.ok) { errors.responseHeaders = hp.error; tabErrors.response = true; }

            // --- response body: validate JSON only if content-type is JSON AND no template tags ---
            var ct = (form.contentType || '').toLowerCase();
            if (ct.indexOf('json') !== -1 && (form.bodyTemplate || '').trim() !== '' && !hasTemplateTags(form.bodyTemplate)) {
                try { JSON.parse(form.bodyTemplate); }
                catch (e) {
                    errors.bodyTemplate = 'Body is not valid JSON: ' + e.message + ' (use {{tags}} for dynamic values).';
                    tabErrors.response = true;
                }
            }

            // --- actions: state writes key safety; webhook url if (the disabled) section has data ---
            (form.stateWrites || []).forEach(function (w, i) {
                if (!w.key || !String(w.key).trim()) {
                    errors['stateWrite.' + i] = 'State key is required.'; tabErrors.actions = true;
                } else if (!/^[A-Za-z0-9_-]{1,64}$/.test(w.key)) {
                    errors['stateWrite.' + i] = 'Key must be A–Z, 0–9, _ or - (max 64).'; tabErrors.actions = true;
                }
            });
            if (form.webhookEnabled && form.webhookUrl && !/^https?:\/\//i.test(form.webhookUrl.trim())) {
                errors.webhookUrl = 'Webhook URL must start with http:// or https://'; tabErrors.actions = true;
            }

            // --- throttling: numeric overrides within bounds (when present) ---
            if (form.latencyOverride !== '' && form.latencyOverride != null) {
                var lo = parseInt(form.latencyOverride, 10);
                if (isNaN(lo) || lo < 0 || lo > 10000) { errors.latencyOverride = 'Latency must be 0–10000 ms.'; tabErrors.throttling = true; }
            }
            if (form.rateLimitOverride !== '' && form.rateLimitOverride != null) {
                var ro = parseInt(form.rateLimitOverride, 10);
                if (isNaN(ro) || ro < 0 || ro > 100000) { errors.rateLimitOverride = 'Rate limit must be 0–100000.'; tabErrors.throttling = true; }
            }
            if (form.priority !== '' && form.priority != null) {
                var pr = parseInt(form.priority, 10);
                if (isNaN(pr) || pr < 0 || pr > 100000) { errors.priority = 'Priority must be 0–100000.'; tabErrors.throttling = true; }
            }

            var count = Object.keys(errors).length;
            return { ok: count === 0, errors: errors, tabErrors: tabErrors, count: count };
        },

        /* Build the frozen MockRuleCreate object (§5.3). Assumes validate() passed
         * for JSON-bearing fields; still parses defensively. */
        serialize: function (form) {
            var hp = parseJsonObject(form.responseHeadersText);
            var responseHeaders = hp.ok ? hp.value : {};

            var match = {
                method: form.method || 'ANY',
                path: (form.path || '/*').trim() || '/*',
                headers: pairsToDict(form.matchHeaders),
                query: pairsToDict(form.matchQuery),
                body_conditions: (form.bodyConditions || [])
                    .filter(function (c) { return c.path && String(c.path).trim(); })
                    .map(function (c) {
                        var o = { path: String(c.path).trim(), op: c.op || 'eq' };
                        if (c.op !== 'exists') o.value = (c.value == null ? '' : String(c.value));
                        else o.value = null;
                        return o;
                    }),
                state_requirements: (form.stateReqs || [])
                    .filter(function (s) { return s.key && String(s.key).trim(); })
                    .map(function (s) {
                        var o = { key: String(s.key).trim(), op: s.op || 'eq' };
                        if (s.op === 'eq' || s.op === 'neq') o.value = (s.value == null ? '' : String(s.value));
                        else o.value = null;
                        return o;
                    }),
            };

            var response = {
                status_code: clampInt(form.statusCode, 100, 599, 200),
                headers: responseHeaders,
                body_template: form.bodyTemplate == null ? '' : String(form.bodyTemplate),
                content_type: form.contentType || 'application/json',
            };

            var stateWrites = (form.stateWrites || [])
                .filter(function (w) { return w.key && String(w.key).trim(); })
                .map(function (w) { return { key: String(w.key).trim(), value: w.value == null ? '' : String(w.value) }; });

            var payload = {
                name: (form.name && form.name.trim()) ? form.name.trim() : null,
                priority: clampInt(form.priority, 0, 100000, 100),
                enabled: form.enabled !== false,
                match: match,
                response: response,
                state_writes: stateWrites,
                latency_ms: (form.latencyOverride === '' || form.latencyOverride == null) ? null : clampInt(form.latencyOverride, 0, 10000, 0),
                rate_limit_per_min: (form.rateLimitOverride === '' || form.rateLimitOverride == null) ? null : clampInt(form.rateLimitOverride, 0, 100000, 0),
                /* AC-33a: webhook_action is accepted-and-stored (no-op in v1). We
                 * still serialize it when the (disabled) sub-section carries data so
                 * the contract never changes later. */
                webhook_action: (form.webhookUrl && form.webhookUrl.trim())
                    ? { url: form.webhookUrl.trim(), body_template: form.webhookBodyTemplate || '' }
                    : null,
            };
            return payload;
        },
    };

    HB.RuleBuilder = RuleBuilder;
})();
