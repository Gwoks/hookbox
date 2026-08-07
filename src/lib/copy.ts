/**
 * copy.md §4–§5 string tables, wired 1:1 (PRD §3: "all user-facing strings are
 * the keys in copy.md §4–§5, wired 1:1 by the FE"). This is the SINGLE source of
 * truth for strings — no copy lives in components. Interpolation slots ({n},
 * {seconds}, {token}, {mock_url}, …) are filled via the `t()` helper.
 *
 * Keys are verbatim from copy.md. Do not paraphrase a value here; a changed
 * string is a copy edit against copy.md.
 */
export const copy = {
  // ── 4.1 Landing ──
  'landing.brand.wordmark': 'HookBox',
  'landing.brand.markAlt': 'HookBox',
  'landing.hero.headline': 'Mock, intercept, and watch HTTP — live.',
  'landing.hero.subhead':
    'Self-hosted API mocking and request interception with a real-time inspector. One email in, one endpoint out. No password, no setup.',
  'landing.email.label': 'Email address',
  'landing.email.placeholder': 'you@company.com',
  'landing.email.submit': 'Get my endpoint',
  'landing.email.submitting': 'Setting up…',
  'landing.email.helper':
    'Your email is the recovery key — no password. Re-enter it any time to pick up where you left off.',
  'landing.strip.mock.title': 'Mock',
  'landing.strip.mock.body':
    'Spin up a mock URL instantly. Define rules by method, path, headers, and body — return exactly the response you want.',
  'landing.strip.intercept.title': 'Intercept',
  'landing.strip.intercept.body':
    "Point an endpoint at a real upstream and proxy what you don't mock. Add latency, rate limits, and chaos to see how your client copes.",
  'landing.strip.inspect.title': 'Inspect',
  'landing.strip.inspect.body':
    'A streaming feed of every hit with a deep inspector — headers, query, body, the response served, and the full resolution trace.',
  'landing.cli.link': 'Already have an endpoint? Tunnel from your machine →',
  'landing.footer.tagline':
    "Self-hosted · Single binary · No tracking — built for developers who'd rather run it themselves.",
  // ── 4.2 Landing states ──
  'landing.error.email.invalid': 'Enter a valid email address.',
  'landing.error.rateLimit': 'Too many attempts. Try again in {seconds}s.',
  'landing.error.network': "Couldn't reach the server. Check your connection and try again.",
  'landing.error.noEndpoint':
    'Session created, but no endpoint came back. Try again — if it persists, restart the server.',
  'landing.warn.storage':
    "This browser won't remember your session (private mode or storage blocked). You can still continue — keep your email to return.",
  'landing.error.generic': "Something didn't go through. Try again.",

  // ── 5.1 App shell & nav ──
  'shell.skipLink': 'Skip to content',
  'shell.nav.endpointSwitcher.label': 'Switch endpoint',
  'shell.nav.endpointSwitcher.placeholder': 'Select an endpoint',
  'shell.nav.newEndpoint': 'New endpoint',
  'shell.nav.newEndpoint.aria': 'Create a new endpoint',
  'shell.nav.theme.aria': 'Toggle theme',
  'shell.account.label': 'Account',
  'shell.account.signedInAs': 'Signed in as {email}',
  'shell.account.signOut': 'Sign out',
  'shell.account.signOut.confirmTitle': 'Sign out?',
  'shell.account.signOut.confirmBody':
    'This clears your secret from this browser. Re-enter your email to return.',
  'shell.account.signOut.confirm': 'Sign out',
  'shell.mobileNav.open.aria': 'Open menu',
  'shell.mobileNav.title': 'Menu',

  // ── 5.2 Dashboard sub-header ──
  'dash.mockUrl.label': 'Mock URL',
  'dash.mockUrl.copy.aria': 'Copy mock URL',
  // Discoverability answer for the removed "Local path" sub-header chip
  // (operator-toolkit F2/AC-86) — the path still lives on Settings → Identity.
  'dash.mockUrl.tooltip':
    "The endpoint's public mock URL. The local /e/ path is on the Settings screen.",
  'dash.pathUrl.label': 'Local path',
  'dash.pathUrl.copy.aria': 'Copy local path',
  'dash.autoCrud.label': 'Auto-CRUD',
  'dash.autoCrud.tooltip': 'Serve a REST store for unmatched requests.',
  'dash.action.rules': 'Rules',
  'dash.action.newRule': 'New rule',
  'dash.action.settings': 'Settings',
  'dash.tunnel.active': 'Tunnel connected',
  'dash.tunnel.active.tooltip': 'A tunnel is forwarding this endpoint to a local machine.',

  // ── 5.3 Feed ──
  'feed.header.title': 'Live feed',
  'feed.pause': 'Pause',
  'feed.resume': 'Resume',
  'feed.pause.aria': 'Pause the live feed',
  'feed.resume.aria': 'Resume the live feed',
  'feed.newCount': '{n} new',
  'feed.newCount.aria': '{n} new requests — click to show',
  'feed.count': 'Showing {n} of last 100',
  'feed.loading.aria': 'Loading recent requests',
  'feed.empty.title': 'No requests yet',
  'feed.empty.body': 'Send a request to your mock URL and it shows up here, live.',
  'feed.empty.sampleLabel': 'Try it',
  'feed.empty.sampleHint': 'Run this in a terminal — or just open the URL in a browser.',
  'feed.empty.sample': 'curl {mock_url}/ping',
  'feed.row.servedBy.aria': 'Served by {servedBy}',
  'feed.row.latency': '{ms}ms',
  'feed.row.select.aria': 'Inspect {method} {path}, {status}',

  // ── 5.15 Feed actions & Clear all (F1, operator-toolkit) ──
  'feed.actions.menu.aria': 'Feed actions',
  'feed.actions.emptyHint': 'Nothing to clear or export — no requests captured yet.',
  'feed.actions.busyHint': 'Finish or cancel the export first.',
  'feed.actions.offlineHint': "You're offline. Reconnect to clear or export.",
  'feed.clearAll': 'Clear all',
  'feed.clearAll.aria': 'Clear all captured requests',
  'feed.clearAll.confirm.title': 'Clear all requests?',
  'feed.clearAll.confirm.body':
    'Deletes every request captured for "{endpoint}" — not only the ones listed here. The feed starts fresh, and this can\'t be undone.',
  'feed.clearAll.confirm.note': 'Requests that arrive after this show up as normal.',
  'feed.clearAll.confirm.confirm': 'Clear all',
  'feed.clearAll.clearing': 'Clearing…',
  'feed.clearAll.error': "Couldn't clear the requests. Nothing was deleted.",

  // ── 5.16 CSV export (F5, operator-toolkit) ──
  'feed.export': 'Export CSV',
  'feed.export.aria': 'Export the listed requests as CSV',
  'feed.export.note': 'Exports the {n} requests listed now, newest first.',
  'feed.export.progress': 'Exporting {done} of {total}…',
  'feed.export.progress.aria': 'Exporting requests',
  'feed.export.announce': 'Exported {done} of {total}.',
  'feed.export.preparing': 'Preparing file…',
  'feed.export.cancel': 'Cancel',
  'feed.export.cancel.aria': 'Cancel the export',
  'feed.export.cancelled': 'Export cancelled. No file was downloaded.',
  'feed.export.done': 'Exported {n} requests.',
  'feed.export.done.partial': 'Exported {n} requests — {m} without detail.',
  'feed.export.error': "Couldn't finish the export. No file was downloaded.",
  'feed.export.error.file':
    "Couldn't create the file. Your browser may have blocked the download, or the export was too large to build.",
  'feed.export.detailNote':
    'Rows whose detail could not be fetched read pending or unavailable in the four detail columns.',

  // ── 5.4 Connection pill ──
  'feed.conn.connecting': 'Connecting…',
  'feed.conn.live': 'Live',
  'feed.conn.reconnecting': 'Reconnecting…',
  'feed.conn.reconnecting.n': 'Reconnecting… (try {n})',
  'feed.conn.sse': 'Live (SSE)',
  'feed.conn.sse.tooltip': 'WebSocket unavailable — streaming over SSE instead. Same events.',
  'feed.conn.offline': 'Offline',
  'feed.conn.offline.tooltip': 'Lost connection to the feed. Showing the last requests received.',
  'feed.conn.unauthorized': 'Not authorized for this feed',
  'feed.conn.unauthorized.tooltip':
    'Your secret no longer authorizes this feed. It may have been rotated elsewhere.',
  'feed.conn.busy': 'Feed busy',
  'feed.conn.busy.tooltip':
    "Too many connections to this endpoint's feed ({max} max). Close another tab and it'll reconnect.",

  // ── 5.5 Inspector ──
  'insp.empty.title': 'Select a request',
  'insp.empty.body': 'Pick a request on the left to inspect it.',
  'insp.loading.aria': 'Loading request detail',
  'insp.pending.title': 'Detail on its way',
  'insp.pending.body': "This request just landed — its detail is still being written.",
  'insp.pending.retry': 'Retry',
  'insp.error.title': "Couldn't load this request",
  'insp.error.body': 'Something went wrong fetching the detail.',
  'insp.error.retry': 'Retry',
  'insp.unauthorized.title': 'Not authorized',
  'insp.unauthorized.body':
    "Your secret doesn't authorize this request. Re-enter your email to continue.",
  'insp.tab.headers': 'Headers',
  'insp.tab.query': 'Query',
  'insp.tab.body': 'Body',
  'insp.tab.response': 'Response Served',
  'insp.tab.trace': 'State & Tracing',
  'insp.headers.empty': 'No headers.',
  'insp.headers.none': 'None',
  'insp.headers.redacted': 'redacted',
  'insp.headers.redacted.tooltip':
    'Sensitive headers (your secret, Cookie, Authorization) are never stored.',
  'insp.query.empty': 'No query parameters.',
  'insp.body.empty': 'No request body.',
  'insp.body.binary': 'Binary body — not shown.',
  // Changed by operator-toolkit copy.md §5: AC-70 stores no truncation marker,
  // so "Truncated at {bytes}" asserted a fact the heuristic doesn't have.
  'insp.body.truncated': 'Capped at {bytes} — this body may be cut short.',
  'insp.body.pretty': 'Pretty',
  'insp.body.raw': 'Raw',
  'insp.body.viewMode.aria': 'JSON view mode',
  'insp.body.largeRaw': 'Large body — showing raw text for speed.',
  'insp.body.notJson': 'Not valid JSON — showing raw text.',
  'insp.body.expandAll': 'Expand all',
  'insp.body.collapseAll': 'Collapse all',
  'insp.body.copy': 'Copy body',
  'insp.response.servedByLabel': 'Served by',
  'insp.response.headers': 'Response headers',
  'insp.response.body': 'Response body',
  'insp.response.empty': 'No response body.',
  'insp.trace.title': 'Resolution trace',
  'insp.trace.empty': 'No trace steps.',
  'insp.trace.stateTitle': 'State at request time',
  'insp.trace.stateEmpty': 'No state set on this endpoint.',
  'insp.trace.latency': 'Applied latency: {applied}ms of {total}ms total',

  // ── 5.6 Served-by chip labels & tooltips ──
  'servedBy.rule.label': 'rule',
  'servedBy.rule.tooltip': 'Matched a rule you wrote.',
  'servedBy.crud.label': 'crud',
  'servedBy.crud.tooltip': 'Served by Auto-CRUD.',
  'servedBy.mitm.label': 'mitm',
  'servedBy.mitm.tooltip': 'Proxied to your upstream target.',
  'servedBy.tunnel.label': 'tunnel',
  'servedBy.tunnel.tooltip': 'Forwarded down your tunnel to localhost.',
  'servedBy.default.label': 'default',
  'servedBy.default.tooltip': "No rule matched — the endpoint's default response.",
  'servedBy.cors.label': 'cors',
  'servedBy.cors.tooltip': 'An Auto-CORS preflight response.',
  'servedBy.chaos.label': 'chaos',
  'servedBy.chaos.tooltip': 'A failure injected by chaos.',
  'servedBy.ratelimit.label': 'ratelimit',
  'servedBy.ratelimit.tooltip': 'Rejected by the rate limit.',

  // ── 5.7 Rule builder ──
  'rule.new.title': 'New rule',
  'rule.edit.title': 'Edit rule',
  'rule.field.name.label': 'Name',
  'rule.field.name.placeholder': 'e.g. Login success',
  'rule.field.name.helper': 'Optional. Just for you — shows in the rules list.',
  'rule.field.priority.label': 'Priority',
  'rule.field.priority.helper': 'Lower wins. Ties break by creation order.',
  'rule.field.enabled.label': 'Enabled',
  'rule.tab.matching': 'Matching',
  'rule.tab.response': 'Response',
  'rule.tab.templating': 'Templating',
  'rule.tab.actions': 'Actions',
  'rule.tab.throttling': 'Throttling',
  'rule.cancel': 'Cancel',
  'rule.save': 'Save rule',
  'rule.saving': 'Saving…',
  'rule.match.method.label': 'Method',
  'rule.match.method.any': 'ANY',
  'rule.match.path.label': 'Path',
  'rule.match.path.placeholder': '/users/:id',
  'rule.match.path.helper': 'Use :name to capture a segment and /* to match the rest. Exact otherwise.',
  'rule.match.path.captured': 'Captures: {names}',
  'rule.match.headers.label': 'Required headers',
  'rule.match.headers.helper': 'Request must include these. Names are case-insensitive.',
  'rule.match.headers.keyPlaceholder': 'Header name',
  'rule.match.headers.valPlaceholder': 'Expected value',
  'rule.match.query.label': 'Required query params',
  'rule.match.query.keyPlaceholder': 'Param',
  'rule.match.query.valPlaceholder': 'Expected value',
  'rule.match.body.label': 'Body conditions',
  'rule.match.body.helper': 'Test the JSON body by path.',
  'rule.match.body.pathPlaceholder': '$.user.role',
  'rule.match.body.op.eq': 'equals',
  'rule.match.body.op.neq': 'not equals',
  'rule.match.body.op.contains': 'contains',
  'rule.match.body.op.exists': 'exists',
  'rule.match.body.valPlaceholder': 'Value',
  'rule.match.state.label': 'State requirements',
  'rule.match.state.helper': "Match only when the endpoint's state holds.",
  'rule.match.state.keyPlaceholder': 'State key',
  'rule.match.state.op.eq': 'equals',
  'rule.match.state.op.neq': 'not equals',
  'rule.match.state.op.exists': 'exists',
  'rule.match.state.op.absent': 'absent',
  'rule.match.state.valPlaceholder': 'Value',
  'rule.match.addRow': 'Add',
  'rule.match.removeRow.aria': 'Remove row',
  'rule.resp.status.label': 'Status code',
  'rule.resp.status.helper': '100–599.',
  'rule.resp.contentType.label': 'Content-Type',
  'rule.resp.headers.label': 'Response headers',
  'rule.resp.body.label': 'Response body',
  'rule.resp.body.placeholder': '{ "id": "{{random \'uuid\'}}" }',
  'rule.resp.body.counter': '{used} / 256 KB',
  'rule.resp.body.counter.over': "{used} / 256 KB — over the cap, this rule won't render",
  'rule.tmpl.intro': 'Click a tag to insert it at your cursor in the response body.',
  'rule.tmpl.honesty': 'Unknown tags are left exactly as written — they never error.',
  'rule.tmpl.group.time': 'Time',
  'rule.tmpl.group.random': 'Random',
  'rule.tmpl.group.request': 'Request',
  'rule.tmpl.group.state': 'State',
  'rule.tmpl.tag.now.iso': "{{now 'iso'}}",
  'rule.tmpl.tag.now.unix': "{{now 'unix'}}",
  'rule.tmpl.tag.random.uuid': "{{random 'uuid'}}",
  'rule.tmpl.tag.random.int': "{{random 'int' 1 100}}",
  'rule.tmpl.tag.request.path': '{{request.path.id}}',
  'rule.tmpl.tag.request.query': '{{request.query.q}}',
  'rule.tmpl.tag.request.header': '{{request.header.Authorization}}',
  'rule.tmpl.tag.request.body': '{{request.body.user.id}}',
  'rule.tmpl.tag.state': '{{state.token}}',
  'rule.act.stateWrites.label': 'State writes',
  'rule.act.stateWrites.helper':
    'Set state when this rule matches. Applied before the body renders, so {{state.key}} sees the new value.',
  'rule.act.stateWrites.keyPlaceholder': 'State key',
  'rule.act.stateWrites.valPlaceholder': 'Value (tags allowed)',
  'rule.act.webhook.label': 'Webhook',
  'rule.act.webhook.badge': 'Stored, not yet sent',
  'rule.act.webhook.helper': "HookBox saves this with the rule but doesn't fire it yet.",
  'rule.act.webhook.urlPlaceholder': 'https://example.com/hook',
  'rule.act.webhook.bodyPlaceholder': 'Webhook body (templated)',
  'rule.thr.intro': "Overrides the endpoint's values for requests this rule serves.",
  'rule.thr.latency.label': 'Latency',
  'rule.thr.latency.unit': 'ms',
  'rule.thr.latency.helper': '0–10000ms. Delays the response.',
  'rule.thr.rateLimit.label': 'Rate limit',
  'rule.thr.rateLimit.unit': 'req/min',
  'rule.thr.rateLimit.helper': '0 = unlimited.',
  'rule.thr.chaosMode.label': 'Chaos mode',
  'rule.thr.chaosMode.inherit': 'Inherit from endpoint',
  'rule.thr.chaosMode.error': 'Error (random 5xx)',
  'rule.thr.chaosMode.dropout': 'Dropout (drop connection)',
  'rule.error.path.invalid': 'Enter a path starting with /.',
  'rule.error.status.range': 'Status must be 100–599.',
  'rule.error.state.key.invalid': 'Keys allow letters, numbers, _ and - (max 64).',
  'rule.error.body.tooLarge': 'Response body is over the 256 KB cap.',
  'rule.footer.invalid': '{n} fields need attention',
  'rule.footer.ready': 'Ready to save',
  'rule.toast.saved': 'Rule saved.',
  'rule.toast.updated': 'Rule updated.',
  'rule.error.save': "Couldn't save the rule. Check the highlighted fields and try again.",
  'rule.error.gone': 'This rule was deleted elsewhere. Closing and refreshing.',

  // ── 5.8 Rules manager ──
  'rules.title': 'Rules',
  'rules.newRule': 'New rule',
  'rules.loading.aria': 'Loading rules',
  'rules.col.priority': 'Priority',
  'rules.col.name': 'Rule',
  'rules.col.match': 'Match',
  'rules.col.enabled': 'On',
  'rules.empty.title': 'No rules yet',
  'rules.empty.body':
    'Without a rule, unmatched requests fall through to Auto-CRUD, then your tunnel, then your proxy target, then the default response.',
  'rules.error.title': "Couldn't load rules",
  'rules.error.retry': 'Retry',
  'rules.row.menu.aria': 'Rule actions',
  'rules.row.edit': 'Edit',
  'rules.row.duplicate': 'Duplicate',
  'rules.row.delete': 'Delete',
  'rules.row.unnamed': 'Untitled rule',
  'rules.delete.title': 'Delete this rule?',
  'rules.delete.body': '"{name}" will stop matching requests. This can\'t be undone.',
  'rules.delete.confirm': 'Delete rule',
  'rules.delete.cancel': 'Cancel',
  'rules.toast.deleted': 'Rule deleted.',
  'rules.toast.duplicated': 'Rule duplicated.',
  'rules.toast.enabled': 'Rule enabled.',
  'rules.toast.disabled': 'Rule disabled.',
  'rules.error.toggle': "Couldn't update the rule. Reverted.",

  // ── 5.20 Default catch-all rule (F6, operator-toolkit) ──
  'rules.default.add': 'Add default rule',
  'rules.default.aria': 'Add a catch-all rule that answers any unmatched request',
  'rules.default.helper':
    'Answers any request no other rule matches — any method, any path — with a 200 and a placeholder JSON body.',
  'rules.default.adding': 'Adding…',
  'rules.default.exists': 'This endpoint already has an enabled catch-all rule.',
  'rules.default.existsDisabled':
    "This endpoint already has a catch-all rule, but it's switched off. Turn it back on instead of adding another.",
  'rules.default.toast': 'Default catch-all rule added.',
  'rules.default.error': "Couldn't add the default rule. Try again.",
  'rules.default.error.duplicate':
    'This endpoint already has a catch-all rule. The list has been refreshed.',
  'rules.default.shadow.title': 'This will take over unmatched requests',
  'rules.default.shadow.body':
    'A catch-all rule answers every request no other rule matches, so the fallbacks below stop being reached.',
  'rules.default.shadow.crud': 'Auto-CRUD stops serving this endpoint.',
  'rules.default.shadow.tunnel': 'Your tunnel stops receiving requests.',
  'rules.default.shadow.proxy': 'Requests stop being proxied to your target URL.',
  'rules.default.shadow.echo': 'The Echo default response stops being used.',
  'rules.default.shadow.recover': 'Switch the rule off or delete it to get them back.',
  'rules.default.shadow.confirm': 'Add rule anyway',
  // The rule's own content — sent to the server, then rendered in the rules
  // list and returned to callers, so it is copy (frozen §5.5.7 as amended).
  'rules.default.ruleName': 'Catch-all (default)',
  'rules.default.bodyTemplate':
    '{\n  "ok": true,\n  "hookbox": "default catch-all",\n  "message": "Edit this rule in HookBox to return your own response."\n}',

  // ── 5.18 Share links — owner (F4, operator-toolkit) ──
  // Written for HASHED share codes (architecture D9/D10/D11): the URL exists
  // only in the 201 response, list rows carry no URL, revoke is by `id`.
  'share.action': 'Share',
  'share.action.aria': 'Share this endpoint read-only',
  'share.action.count.aria': 'Share this endpoint read-only — {n} active links',
  'share.title': 'Share a read-only link',
  'share.intro':
    "A share link lets anyone holding the URL read this endpoint's recent requests in a browser. No account, no sign-in, and nothing they can change.",
  'share.warning.title': 'A share link publishes captured traffic',
  'share.warning.body':
    "Anyone with the URL can read this endpoint's last 100 requests — including ones that arrived before you created the link, and ones other people sent. Each one shows its method, path, status, headers, query and body, plus the response headers and body HookBox returned. The endpoint's name is visible too.",
  'share.warning.redaction':
    "Hidden automatically: Authorization, Cookie and X-Owner-Id request headers, and any Set-Cookie or Authorization response header. Nothing else is hidden — don't share an endpoint that carries production secrets.",
  'share.create': 'Create share link',
  'share.creating': 'Creating…',
  'share.label.label': 'Label',
  'share.label.placeholder': 'e.g. Acme support ticket #421',
  'share.label.helper':
    "Optional, but worth it — the label and the date are all you'll have to tell links apart later. Only you see it.",
  'share.label.tooLong': 'Labels are 80 characters or fewer.',
  'share.created.title': 'Your share link',
  'share.created.onceHint':
    "Shown once — copy it now. HookBox keeps only a fingerprint of the link, so it can't show it again.",
  'share.created.lostHint': 'If you lose it, revoke the link and create a new one.',
  'share.created.copy.aria': 'Copy the share link',
  'share.created.open': 'Open in new tab',
  'share.created.open.aria':
    'Open the share link in a new tab — this is exactly what the recipient sees',
  'share.created.localWarning':
    'This link points at {origin}, which may not be reachable from outside your network. Set PUBLIC_BASE_URL on the server to mint links other people can open.',
  'share.list.title': 'Active links',
  'share.list.count': '{n} of {max}',
  'share.list.hint':
    "A link's URL is never shown again after it's created — the label and date are how you tell them apart.",
  'share.list.loading.aria': 'Loading share links',
  'share.list.empty.title': 'No share links yet',
  'share.list.empty.body':
    'Create one when you need to show someone what their webhook actually sent.',
  'share.list.error.title': "Couldn't load share links",
  'share.list.error.body': 'Something went wrong reaching the server.',
  'share.row.untitled': 'Untitled link',
  'share.row.created': 'Created {when}',
  'share.row.lastUsed': 'Opened {when}',
  'share.row.neverUsed': 'Never opened',
  'share.row.lastUsed.tooltip':
    'Recorded at most once a minute, so a very recent open may not show yet.',
  'share.row.revoke': 'Revoke',
  'share.row.revoke.aria': 'Revoke this share link',
  'share.row.revoke.confirm': 'Revoke this link?',
  'share.row.revoke.confirmHint':
    "It stops working immediately for everyone who has the URL, and it can't be brought back.",
  'share.row.revoke.confirmAction': 'Revoke',
  'share.limit.reached': 'You have the maximum of {max} active links. Revoke one to create another.',
  'share.toast.created': 'Share link created.',
  'share.toast.revoked': 'Share link revoked.',
  'share.toast.revokedAlready': 'That link was already revoked.',
  'share.error.create': "Couldn't create the link. Try again.",
  'share.error.revoke': "Couldn't revoke the link. It's still active — try again.",
  'share.done': 'Done',

  // ── 5.19 Public shared view — /s/:code (F4, operator-toolkit) ──
  // Every string here is read by someone with no session and no HookBox
  // context (copy.md §1.1 "stranger-facing" register). Zero owner vocabulary.
  'viewer.docTitle': 'Shared requests · HookBox',
  'viewer.title': 'Shared requests',
  'viewer.banner.title': 'Read-only shared view',
  'viewer.banner.body':
    "Someone shared one HookBox endpoint's recent requests with you. You're not signed in, and nothing on this page can be changed.",
  'viewer.readOnlyChip': 'Read-only',
  'viewer.subject.name': 'Endpoint: {name}',
  'viewer.subject.unnamed': 'Unnamed endpoint',
  'viewer.subject.since': 'Capturing since {when}',
  'viewer.subject.total': '{n} requests received in total',
  'viewer.count': 'Showing {n} of the last 100',
  'viewer.updated': 'Updated {when}',
  'viewer.updating':
    'Shows the last 100 requests from the past 24 hours. Updates every 5 seconds while this tab is open.',
  'viewer.updating.paused':
    'Paused while this tab is in the background. Switch back to it to resume.',
  'viewer.refresh': 'Refresh',
  'viewer.refresh.aria': 'Refresh the list',
  'viewer.newRequests.aria': '{n} new requests',
  'viewer.loading.aria': 'Loading shared requests',
  'viewer.empty.title': 'No requests to show',
  'viewer.empty.body':
    "This page shows an endpoint's last 100 requests from the past 24 hours. Either nothing has arrived yet, or older requests have already rolled off. It refreshes on its own — you can leave the tab open.",
  'viewer.row.expand.aria': 'Show detail for {method} {path}, {status}',
  'viewer.row.collapse.aria': 'Hide detail for {method} {path}',
  'viewer.detail.loading.aria': 'Loading request detail',
  'viewer.detail.gone.title': 'This request is no longer available',
  'viewer.detail.gone.body':
    'HookBox keeps the last 100 requests for 24 hours. This one has rolled off. The rest of the list still works.',
  'viewer.detail.error.title': "Couldn't load this request",
  'viewer.detail.error.body': 'Something went wrong fetching the detail.',
  'viewer.headers.redacted.tooltip':
    "HookBox hides credential headers in shared views. This value isn't shown.",
  'viewer.unavailable.title': "This link isn't available",
  'viewer.unavailable.body':
    'It may have been revoked, or it may never have existed. Ask whoever sent it for a new one.',
  'viewer.unavailable.about': 'What is HookBox?',
  'viewer.rateLimited.title': 'Too many requests',
  'viewer.rateLimited.body':
    'This page has been loaded too often from your network. It retries in {seconds}s.',
  'viewer.error.title': "Couldn't load the shared requests",
  'viewer.error.body':
    'Something went wrong reaching the server. Anything already listed may be out of date.',
  'viewer.offline.title': "You're offline",
  'viewer.offline.body': 'This page will catch up when your connection returns.',
  'viewer.footer':
    'Served by HookBox — self-hosted API mocking and request inspection. This page is read-only.',
  // The viewer's column-header band (hidden below `sm`) — no copy.md key was
  // drafted for these; short, neutral table-header labels.
  'viewer.col.method': 'Method',
  'viewer.col.path': 'Path',
  'viewer.col.status': 'Status',
  'viewer.col.served': 'Served',
  'viewer.col.duration': 'Ms',
  'viewer.col.when': 'When',

  // ── 5.9 Endpoint settings ──
  'set.title': 'Settings',
  'set.loading.aria': 'Loading settings',
  'set.error.title': "Couldn't load settings",
  'set.error.retry': 'Retry',
  'set.save': 'Save',
  'set.saving': 'Saving…',
  'set.toast.saved': 'Settings saved.',
  'set.error.save': "Couldn't save. Your changes were reverted.",
  'set.identity.title': 'Identity',
  'set.identity.name.label': 'Endpoint name',
  'set.identity.name.placeholder': 'e.g. Checkout API',
  'set.identity.name.helper': 'Optional. Just a label for you.',
  'set.identity.mockUrl.label': 'Mock URL',
  'set.identity.pathUrl.label': 'Local path',
  'set.identity.token.label': 'Token',
  'set.proxy.title': 'Proxy target (MITM)',
  'set.proxy.url.label': 'Target URL',
  'set.proxy.url.placeholder': 'https://api.example.com',
  'set.proxy.url.helper':
    'Unmatched requests forward here. A matching rule always wins. Leave empty to turn off.',
  'set.proxy.url.note': 'Targets on private or loopback addresses are blocked and return 502.',
  'set.proxy.url.invalid': 'Enter an http or https URL with a host.',
  'set.crud.title': 'Auto-CRUD',
  'set.crud.toggle.label': 'Serve a REST store for unmatched requests',
  'set.crud.peek.label': 'Collections',
  'set.crud.peek.placeholder': 'Collection name',
  'set.crud.peek.view': 'View',
  'set.crud.peek.empty': 'This collection is empty.',
  'set.crud.peek.count': '{n} items',
  'set.crud.peek.invalid': 'Names allow letters, numbers, _ and - (max 64).',
  'set.crud.peek.clear': 'Clear collection',
  'set.default.title': 'Default response',
  'set.default.helper': 'What an endpoint returns when nothing else matches.',
  'set.default.mock404.label': '404 Not Found',
  'set.default.mock404.helper': 'Return 404 with a no-match body.',
  'set.default.echo.label': 'Echo',
  'set.default.echo.helper': 'Reflect the request back as JSON (method, path, query, headers, body).',
  'set.cond.title': 'Simulated conditions',
  'set.cond.helper': 'Applied in order: rate limit → chaos → latency.',
  'set.cond.latency.label': 'Latency',
  'set.cond.latency.unit': 'ms',
  'set.cond.latency.helper': '0–10000ms added before responding.',
  'set.cond.rateLimit.label': 'Rate limit',
  'set.cond.rateLimit.unit': 'req/min',
  'set.cond.rateLimit.helper': '0 = unlimited. Over the limit returns 429. Counts proxy and Auto-CRUD too.',
  'set.cond.chaos.label': 'Chaos',
  'set.cond.chaos.unit': '%',
  'set.cond.chaos.helper': 'Chance a request fails on purpose. 0 never, 100 always.',
  'set.cond.chaosMode.label': 'Chaos mode',
  'set.cond.chaosMode.error': 'Error (random 5xx)',
  'set.cond.chaosMode.dropout': 'Dropout (drop the connection)',
  'set.cors.title': 'Auto-CORS',
  'set.cors.toggle.label': 'Handle CORS automatically',
  'set.cors.helper': 'Answers OPTIONS preflight and reflects the Origin — on the mock surface only.',

  // ── 5.17 Configuration export / import (F3, operator-toolkit) ──
  'set.config.title': 'Configuration',
  'set.config.helper':
    "Move this endpoint's settings and rules to another endpoint, or keep a copy next to your code.",
  'set.config.export': 'Export config',
  'set.config.export.aria': "Download this endpoint's configuration as JSON",
  'set.config.export.busy': 'Exporting…',
  'set.config.export.dirty': 'Exports the saved configuration — save your changes first to include them.',
  'set.config.export.error': "Couldn't build the export. Nothing was downloaded.",
  'set.config.export.error.rules': "Couldn't read this endpoint's rules, so no file was created. Try again.",
  'set.config.toast.exported': 'Configuration exported.',
  'set.config.import': 'Import config…',
  'set.config.import.aria': 'Choose a HookBox config file to import',
  'set.config.import.helper':
    'Reads a HookBox config file. Settings are replaced; rules are added to the ones already here.',
  'set.config.import.fileHint': 'One .json file, up to 5 MB and 200 rules.',
  'set.config.import.reading': 'Checking the file…',
  'set.config.import.invalid.json':
    "That file isn't valid JSON. If you edited it by hand, check for a missing brace or a trailing comma.",
  'set.config.import.invalid.empty': 'That file is empty.',
  'set.config.import.invalid.shape': "That file isn't a HookBox config: {reason}",
  'set.config.import.invalid.field': '{field} is invalid: {reason}',
  'set.config.import.invalid.rule': 'Rule {index} is invalid: {reason}',
  'set.config.import.wrongVersion': 'Unsupported config version {version}. This build reads version 1.',
  'set.config.import.tooLarge': 'That file is larger than 5 MB.',
  'set.config.import.tooManyRules': 'That file has {n} rules — the limit is 200.',
  'set.config.import.chooseAnother': 'Choose another file',
  'set.config.confirm.title': 'Apply this configuration?',
  'set.config.confirm.exported': 'Exported {when} from an endpoint named "{name}".',
  'set.config.confirm.exported.unnamed': 'Exported {when} from an unnamed endpoint.',
  'set.config.diff.title': 'Settings that change ({n})',
  'set.config.diff.none': 'No settings change — the file matches this endpoint.',
  'set.config.diff.unchangedNote': "Settings that stay the same aren't listed.",
  'set.config.diff.change.aria': '{field}: {from} becomes {to}',
  'set.config.diff.empty': 'not set',
  'set.config.diff.targetUrl.warning':
    'target_url changes where unmatched requests are proxied, the moment you apply this. Any active share link starts showing that upstream\'s responses.',
  'set.config.confirm.rules': 'Adds {n} rules to the {existing} already on this endpoint. Nothing in the rules list is replaced or deleted.',
  'set.config.confirm.rules.none': 'This file has no rules to add.',
  'set.config.confirm.headerTagWarning':
    "{n} rules in this file copy request headers into their responses. Anything a caller sends in a header — including credentials — can come back in the body.",
  'set.config.confirm.dirty': 'You have unsaved changes on this screen. Applying this discards them.',
  'set.config.confirm.confirm': 'Apply configuration',
  'set.config.import.progressConfig': 'Applying settings…',
  'set.config.import.progressRules': 'Creating rule {i} of {n}…',
  'set.config.import.announce': 'Created {i} of {n} rules.',
  'set.config.import.dontClose': 'Keep this tab open until it finishes.',
  'set.config.toast.imported': 'Configuration imported.',
  'set.config.import.done': 'Configuration applied — {n} rules added.',
  'set.config.import.done.noRules': 'Configuration applied. This file had no rules.',
  'set.config.import.failedConfig': "Couldn't apply the settings, so no rules were created. The server said: {detail}",
  'set.config.import.failedRule':
    'Settings were applied and {done} of {total} rules were created. Rule {index} ("{name}") failed: {detail} No rule after it was attempted, and nothing was rolled back.',
  'set.config.import.viewRules': 'View rules',

  'set.retention.title': 'Retention & state',
  'set.retention.note': 'The last 100 requests per endpoint are kept for 24 hours.',
  'set.retention.clearHistory': 'Clear request history',
  'set.retention.clearState': 'Clear state',
  'set.retention.stateNote': 'State is per-endpoint and shared across all callers.',
  'set.danger.title': 'Danger zone',
  'set.danger.delete.label': 'Delete endpoint',
  'set.danger.delete.helper': 'Permanently delete this endpoint, its rules, state, and history.',
  'set.confirm.clearHistory.title': 'Clear request history?',
  // Changed by operator-toolkit copy.md §5: {n} was fed from a LIFETIME
  // request_count while at most TRACE_CAP=100 traces are ever stored, so the
  // old sentence was wrong for any endpoint past 100 hits (AC-77).
  'set.confirm.clearHistory.body':
    "Deletes every request captured for this endpoint. The live feed starts fresh, and this can't be undone.",
  'set.confirm.clearHistory.confirm': 'Clear history',
  'set.confirm.clearState.title': 'Clear endpoint state?',
  'set.confirm.clearState.body':
    "Every state key on this endpoint will be removed. Rules that require state stop matching until it's set again.",
  'set.confirm.clearState.confirm': 'Clear state',
  'set.confirm.clearCollection.title': 'Clear collection "{name}"?',
  'set.confirm.clearCollection.body':
    'Every item in this Auto-CRUD collection will be removed. This can\'t be undone.',
  'set.confirm.clearCollection.confirm': 'Clear collection',
  'set.confirm.delete.title': 'Delete endpoint {token}?',
  'set.confirm.delete.body':
    'This deletes the endpoint and everything in it — rules, state, history, and collections. The mock URL stops working. This can\'t be undone.',
  'set.confirm.delete.prompt': 'Type the token to confirm:',
  'set.confirm.delete.placeholder': '{token}',
  'set.confirm.delete.confirm': 'Delete endpoint',
  'set.confirm.cancel': 'Cancel',
  'set.toast.historyCleared': 'Request history cleared.',
  'set.toast.stateCleared': 'State cleared.',
  'set.toast.collectionCleared': 'Collection cleared.',
  'set.toast.deleted': 'Endpoint deleted.',
  'set.error.delete': "Couldn't delete the endpoint. Try again.",

  // ── 5.10 Dashboard shell states ──
  'dash.state.loading.aria': 'Loading endpoint',
  'dash.state.notFound.title': 'Endpoint not found',
  'dash.state.notFound.body':
    "This endpoint doesn't exist — it may never have, or it was deleted long ago.",
  'dash.state.gone.title': 'Endpoint deleted',
  'dash.state.gone.body': 'This endpoint was deleted. Its mock URL now returns 410.',
  'dash.state.backToStart': 'Back to start',
  'dash.state.offline.title': "You're offline",
  'dash.state.offline.body':
    "Lost the connection. Showing the last data received — we'll catch up when you're back.",

  // ── 5.11 Tunnel / CLI page ──
  'cli.title': 'Tunnel from your machine',
  'cli.intro':
    'The tunnel forwards public traffic for an endpoint to a server on your localhost — so you can test webhooks and integrations against code you\'re running right now. Tunneled requests show up in the feed labeled tunnel.',
  'cli.command.label': 'Run this',
  'cli.command.template':
    'tunnel --port {port} --endpoint {token} --secret {secret} --host {host}',
  'cli.command.portDefault': '3000',
  'cli.command.copy.aria': 'Copy tunnel command',
  'cli.secret.reveal': 'Reveal secret',
  'cli.secret.hide': 'Hide secret',
  'cli.secret.warning':
    "This is your owner capability — your secret. Anyone who has it controls this endpoint. Don't paste it into shared terminals, screenshots, or issues.",
  'cli.behavior.title': 'What to expect',
  'cli.behavior.order':
    'In the resolution order, the tunnel is tried after Auto-CRUD and before your proxy target.',
  'cli.behavior.noTunnel':
    "When nothing is connected, tunneled paths return 504 no_tunnel — that's expected, not a bug.",
  'cli.behavior.takeover':
    'Only one tunnel binds at a time. Connect a second and it takes over; the first is dropped with "rebound elsewhere."',
  'cli.behavior.reconnect':
    "If the connection drops, the CLI reconnects with backoff. Bad credentials stop it — it won't retry into a wall.",
  'cli.toast.copied': 'Copied.',

  // ── 5.13 Global toasts & shared ──
  'common.copy': 'Copy',
  'common.copied': 'Copied',
  'common.copy.announce': 'Copied to clipboard',
  'common.copy.failed': "Couldn't copy. Select the text and copy it manually.",
  'common.retry': 'Retry',
  'common.cancel': 'Cancel',
  'common.save': 'Save',
  'common.close': 'Close',
  'common.close.aria': 'Close',
  'common.dismiss': 'Dismiss',
  'common.error.401':
    'Your session ended — your secret was rotated somewhere else. Enter your email to continue.',
  'common.error.network': 'Network error. Check your connection and try again.',
  'common.error.generic': 'Something went wrong. Try again.',
  // Shared 404/410 mid-session mapping (operator-toolkit AC-81) — the
  // endpoint was deleted in another tab while this one still had it open.
  'common.error.endpointGone': 'This endpoint no longer exists. Reload the page to catch up.',
  'common.notFound.title': 'Page not found',
  'common.notFound.body': "That page doesn't exist.",
  'common.notFound.home': 'Back to start',
} as const

export type CopyKey = keyof typeof copy

/** Resolve a copy key, substituting {slot} interpolation values. */
export function t(key: CopyKey, vars?: Record<string, string | number>): string {
  let s: string = copy[key]
  if (vars) {
    for (const [k, v] of Object.entries(vars)) {
      s = s.replaceAll(`{${k}}`, String(v))
    }
  }
  return s
}
