# Copy & content design: Operator Toolkit (F1–F7)

- **Slug:** `operator-toolkit`
- **Status:** Draft (copywriter-engineer, DESIGN phase)
- **Owns:** every word a user reads on the six new/changed surfaces in this batch, plus the first
  words a *stranger* ever reads from HookBox (`/s/:code`).
- **Upstream (read, in this order):** `ux.md` (structure, states, its own §4 key proposal),
  `design.md` (visual voice + the copy gaps it raised in §10/§11), `journey.md` (every flow, error and
  edge state), `prd.md` (§4 ACs, §5 frozen contract), `architecture.md` (D9/D10/D11 — **hashed** share
  codes), `security.md` (AC-S1, AC-S11, AC-S21), `src/lib/copy.ts` (the live table + its conventions).
- **Downstream consumer:** `frontend-engineer` wires each key → string **1:1** into
  `src/lib/copy.ts`. This file is the single source of truth for wording (AC-64); no copy lives in
  components.

> **How to read this doc.** §1 is the voice contract (what's inherited, and the three *new* registers
> this batch introduces). §2 is content design / IA per surface — order, hierarchy, what to omit.
> §3 is the public/marketing surface (the viewer page is HookBox's only outward-facing page besides
> the landing, plus optional landing deltas). §4 is the string tables: flat **key → string**, grouped
> by feature, numbered as new sections **§5.15–§5.21** continuing
> `docs/features/hookbox-rust-replatform/copy.md`. §5 lists **changed** existing strings, §6 the keys
> reused verbatim (and the ones from ux.md §4 I dropped), §7 flags for the PM, §8 the coverage map
> (every journey.md state → a key).
>
> **Conventions, matched to the live table exactly:** flat dot-namespaced keys
> (`feed.clearAll.confirm.body`), sentence case, second person, no exclamation marks, em dashes for
> asides, `…` for in-progress, `{slot}` interpolation via `t()`, `.aria` suffix for accessible names
> that differ from the visible label. Terminology is locked to §5 / `FEATURES.md`: **endpoint · token
> · owner capability ("your secret") · rule · trace · state · Auto-CRUD · proxy / MITM · tunnel ·
> served-by · share link**. Anything I wrote that implies behaviour the frozen contract doesn't
> guarantee is in **§7**.

---

## 1. Voice & tone

**Voice in one line:** *a sharp colleague handing you the right tool — precise, unhurried, dev-to-dev;
says what happened, what it costs, and what to do next, and never sells, softens, or apologises
twice.*

That is the shipped voice (replatform copy.md §1) and nothing here changes it. What this batch adds is
three registers HookBox has never had to write in before. Each has a rule.

### 1.1 The three new registers

| Register | Where | Rule |
|---|---|---|
| **Destructive-but-cheap** | F1 Clear all, F3 import confirm | Serious, specific, *proportionate*. Traces are ephemeral by design (100 rows, 24 h). Name the blast radius, say it can't be undone, then stop. No capital letters, no "permanently", no typed-token friction — that is reserved for deleting an endpoint. |
| **Security disclosure** | F4 mint step | Exact and complete, never alarming. Enumerate what becomes public *before* the link exists, in plain nouns, in the order a reader cares about (who can see it → what they see → what stays hidden → the one thing not to do). No scare words ("danger", "warning:", "⚠️ CAUTION"); the amber alert already carries the weight. |
| **Stranger-facing** | F4 `/s/:code` viewer | Explain the *object*, never the protocol. The reader may be a vendor's support engineer, a PM, or a QA on a phone — assume they know what an HTTP request is, assume they have never heard of HookBox. Every sentence answers one of: what is this, who sent it, can I break it, why is it changing. Zero owner vocabulary: no "endpoint token", no "mock URL", no "your secret", no imperative that implies control. |

### 1.2 Tone shifts (extends the shipped table)

| Moment | Tone | Shape |
|---|---|---|
| **Destructive confirm** | plain, names the target, one consequence sentence | "Deletes every request captured for "checkout-api" — not only the ones listed here." |
| **Irreversible + already distributed** (revoke) | flat and complete; no reassurance we can't give | "It stops working immediately for everyone who has the URL, and it can't be brought back." |
| **Shown-once secret** | urgent but calm; say *why* it can't be re-shown | "Shown once — copy it now. HookBox keeps only a fingerprint of the link, so it can't show it again." |
| **Long-running progress** | numeric, present tense, no adjectives | "Exporting 41 of 100…" → "Preparing file…" |
| **Partial failure** | forensic. Five facts, no hedging, no blame | applied? / how many landed / which one failed / why / what was *not* attempted |
| **Anonymous arrival** | welcoming-neutral, orienting, zero pressure | "Someone shared one HookBox endpoint's recent requests with you." |
| **Aged-out data** | matter-of-fact, never an error | "HookBox keeps the last 100 requests for 24 hours. This one has rolled off." |

### 1.3 Do / Don't (this batch)

- **Clear-all confirm.** ✅ "Deletes every request captured for "{endpoint}" — not only the ones listed here. The feed starts fresh, and this can't be undone." ❌ "All 12,481 traces will be permanently destroyed. This action is irreversible!" (the number is wrong — see §7.5 — and the tone is wrong for a debugging log).
- **Mint disclosure.** ✅ "Anyone with the URL can read this endpoint's last 100 requests — including ones that arrived before you created the link, and ones other people sent." ❌ "Share links are secure and only visible to people you trust."
- **Shown-once.** ✅ "Shown once — copy it now." ❌ "Save this somewhere safe!" (vague), ❌ "You can reopen this dialog to copy it again." (false under hashed storage).
- **Revoke.** ✅ "Couldn't revoke the link. It's still active — try again." ❌ "Revoke failed." (on a security action, "failed" without state is the ambiguity).
- **Viewer banner.** ✅ "You're not signed in, and nothing on this page can be changed." ❌ "Read-only mode — some features are unavailable." (implies a locked app, i.e. exactly the "dashboard with pieces missing" reading design.md forbids).
- **Viewer empty.** ✅ "Either nothing has arrived yet, or older requests have already rolled off." ❌ "No requests yet." (wrong half the time — the link may be a day old).
- **Catch-all rule.** ✅ "Answers any request no other rule matches with a 200 and a placeholder JSON body." ❌ "Adds a smart default response." (it is neither smart nor a default *mode*).
- **Import.** ✅ "Settings are replaced; rules are added to the ones already here." ❌ "Restores your configuration." (nothing is restored — rules accumulate).
- **Progress.** ✅ "Preparing file…" ❌ "Almost done!" / "Hang tight…"

### 1.4 Terminology additions (locked)

| Use | Never |
|---|---|
| share link (the thing) · the URL (the string) | public link, guest link, snapshot link, invite |
| revoke | delete, disable, deactivate, kill |
| viewer / whoever has the URL | guest, visitor, public user, recipient (recipient is fine in prose, never a noun in UI) |
| read-only shared view | preview, public dashboard, live share |
| config / configuration file | backup, snapshot, profile, preset |
| catch-all rule | wildcard rule, fallback rule, default mode (that's `default_mode`, a different thing) |
| detail (a request's headers/query/bodies) | payload, metadata |
| rolled off | expired, purged, deleted (for TTL/cap eviction — the owner didn't do it) |

Capitalisation unchanged: **HookBox**, **Auto-CRUD**, **Auto-CORS** are proper; `served_by` values and
config field names (`target_url`, `auto_crud`) stay lowercase mono — they are data, not prose.

---

## 2. Content design / IA — per surface

For each surface: what to say, in what order, and what to leave out. Structure follows ux.md §2 and
design.md §3; nothing here changes either.

### 2.1 Live-feed action menu + clear-all confirm (F1)

- **Menu order is copy-load-bearing:** `Export CSV` → separator → `Clear all`. Destructive last, and
  it is the only red word in the pane.
- **Hint line under the items** carries exactly one reason the items are dead — empty feed, export in
  flight, or offline. Never stack two.
- **Confirm hierarchy:** title (a question) → one body sentence that widens the blast radius beyond
  the visible list → *no count*. Then Cancel / Clear all.
- **Omit:** any number. `rows.length` drifts while the dialog is open and `request_count` is a
  lifetime counter, not the ≤100 stored traces (ux.md gap #3, journey.md gap 18/20). The honest,
  cheap, always-true phrasing is "every request captured for this endpoint".
- **Omit:** "are you sure", the word "permanently", and a typed-token step.
- Success is a toast reusing `set.toast.historyCleared`; failure keeps the dialog open and puts the
  server's `detail` inside it, because the operator may need to read and retry.

### 2.2 CSV export strip (F5)

- **Two lines of copy total.** Line 1 is a counter (`Exporting 41 of 100…`), which is also the
  `aria-valuetext`. Line 2 doesn't exist — the bar is the second line.
- **The phase change is the message.** `Exporting {done} of {total}…` → `Preparing file…` at 100 %.
  Without that swap the UI reads as hung during multi-second serialisation of up to ~50 MB of body
  text (design.md §3.3, ux.md gap #10).
- **The menu item's note sets the snapshot expectation** ("Exports the {n} requests listed now,
  newest first") because the feed keeps growing after the click.
- **Cancel says `Cancel`, not `Cancel export`** — it lives inside a strip that already says what is
  being exported; the accessible name carries the object.
- **Completion is a toast with a count**, and a *second* number only when detail was missing.
- **Omit:** column documentation, byte counts, ETA (we can't compute one honestly), and any promise
  the file matches the feed *after* the click.
- **The CSV's own sentinels** (`pending` / `unavailable`) are frozen artifact values, not copy — see
  §4.3 and §7.13.

### 2.3 Settings → Configuration (F3)

- **Section hierarchy:** `h4` title → one *why* sentence → the two buttons → one *what happens*
  sentence under them. That's the same silhouette as Retention & state, so the band reads as another
  operation rather than a new concept.
- **Say "replaced" and "added" in the same breath, always.** The single most surprising thing about
  import (AC-18) is that rules accumulate while settings overwrite. Every place import is described —
  helper, confirm, success — carries both verbs.
- **Import confirm order (security.md AC-S21):** what this is (the file's provenance) → what changes
  (the field-by-field diff, old→new) → the `target_url` consequence if present → how many rules get
  added → the dirty-form warning if the form has unsaved edits → `Apply configuration`.
- **The diff shows only changing fields**, with a one-line note that unchanged ones are omitted, so a
  9-row wall doesn't hide the one row that matters. Field names stay in mono, verbatim
  (`target_url`) — they're data, and paraphrasing them into prose labels would break the operator's
  ability to match them against the file they're holding.
- **Partial failure is a persistent alert, never a toast.** It carries five facts in one order:
  settings applied → how many rules landed → which rule failed and why → nothing after it was
  attempted → nothing was rolled back. Plus one action: `View rules`.
- **Omit:** the word "backup" or "restore", an undo offer (there is none — journey F3-D), progress
  percentages for the config `PATCH` (it's one request), and any suggestion that import creates a new
  endpoint.

### 2.4 Share dialog — owner (F4)

- **Reading order is a funnel and the copy has to earn each step:** what a share link *is* (one
  sentence, in the dialog description) → what it *exposes* (the amber alert, above the button, always
  visible) → the label field → `Create share link` → the shown-once panel → the inventory of active
  links → `Done`.
- **The disclosure is complete or it is worthless.** security.md AC-S11 makes it a MUST, and it must
  name: who can see it (anyone with the URL, no sign-in), the window (last 100 requests), that it
  includes requests from *before* the link existed and from *other people*, request headers/query/
  body, response headers *and* body, and the endpoint's name. Then, separately, what HookBox hides.
  Two paragraphs, because one 90-word block doesn't get read.
- **The label is the identity of the link.** Under hashed storage (architecture D11) the list has no
  URL — label + date is all the operator gets. So the helper text *nudges* rather than shrugs:
  "Optional, but worth it."
- **The shown-once panel explains the mechanism, briefly.** "HookBox keeps only a fingerprint of the
  link" is a one-clause explanation of why we can't show it again; without it, "shown once" reads as
  an arbitrary product decision and the operator assumes they can come back.
- **`Open in new tab` lives only in that panel** — it is the only place the URL exists, and "see
  exactly what they see" is the most reassuring control in the dialog.
- **Revoke is a two-step inside the row**, and its consequence sentence names *other people*: the URL
  is already in a ticket somewhere.
- **Omit:** the code, a code prefix, or any per-row URL (they don't exist). Omit "secure", "private",
  "expires" (nothing expires), an email-the-link action, and any count of how many people opened it
  (we only store a coarse last-opened timestamp).

### 2.5 Public shared view — `/s/:code` (F4)

This page has ~8 seconds to answer four questions before anyone scrolls. The copy answers them in
this exact order:

1. **What is this?** → the standing banner title, `Read-only shared view`.
2. **Who sent it and why am I here?** → banner body, "Someone shared one HookBox endpoint's recent
   requests with you."
3. **Can I break anything?** → same sentence, second clause: "nothing on this page can be changed."
   Reinforced by the `Read-only` chip next to the data (the banner scrolls away; the chip doesn't).
4. **Why is it moving / why is it empty?** → the caption under the card, which states the retention
   window *and* the refresh cadence in one line.

- **The `<h1>` is the static string `Shared requests`, always.** The endpoint name is
  operator-authored text on an unauthenticated page — a ready-made phishing slot — so it never
  occupies the most authoritative line, and `document.title` is static too (ux.md gap #16).
- **The empty state must cover two causes with one message** (journey.md E2/E3/E8, gap 13): never
  arrived, or already rolled off. "No requests yet" would tell a day-late recipient the link is
  broken.
- **A detail 404 must not read like a dead link.** `viewer.detail.gone.body` ends with "The rest of
  the list still works." — the one sentence that defuses the AC-36 ambiguity for a human (journey.md
  BLOCKER 1).
- **One message for unknown + revoked + deleted**, permanently. Never add a "this link was revoked"
  variant; that reintroduces the oracle AC-36 exists to remove.
- **Omit, on this page, forever:** the token, the mock URL, `/e/` paths, curl samples, rules, config,
  the word "your", any imperative that implies control ("Pause", "Clear", "Export"), any owner-voiced
  tooltip (`servedBy.*.tooltip` says "**your** upstream target"), and any off-origin link (the code
  is a bearer credential in the path).
- The footer is the only brand line: what HookBox is, and that this page is read-only. It repeats
  the read-only claim on purpose — it is the last thing a skimmer sees.

### 2.6 Rules Manager — Add default rule (F6)

- **Two placements, one helper sentence, and the helper is the honesty.** "Answers any request no
  other rule matches with a 200 and a placeholder JSON body" says *any method, any path, placeholder
  body* without a spec dump. Nobody should expect it to be smart.
- **The response body itself is copy** (§4.6) and must be self-describing: a reader who hits the
  endpoint from curl and gets this body should immediately know where it came from and how to change
  it.
- **Disabled state explains itself**, and the reason is reachable by keyboard (ux.md gap #26).
- **The shadowing warning, if the PM accepts it** (journey.md BLOCKER 36), is a confirm with one lead
  sentence plus one bullet per fallback that actually applies — Auto-CRUD, tunnel, proxy target, Echo
  — and a closing line on how to get them back. Conditional bullets, never a paragraph listing
  things that may not be switched on.
- **Omit:** priority mechanics (the list already shows priority), matcher syntax, and a second CTA in
  the empty state — `New rule` is in the toolbar directly above.

### 2.7 Inspector / body rendering (F7)

Three captions, all quiet, all `text-caption text-text-tertiary`, none an alert — nothing is wrong,
the tool just made a choice:

- large body → showing raw text (a *performance* choice, stated as such);
- not valid JSON → showing raw text (a *fact* about the body, not a failure);
- at the capture cap → *may* be cut short. The copy hedges because the heuristic hedges: `len ==
  MAX_BODY_BYTES` cannot distinguish "exactly cap" from "truncated at cap" (PRD R4). Writing
  "Truncated at 256 000" would state something we don't know.

---

## 3. Public / marketing copy

### 3.1 `/s/:code` is a marketing surface (whether we like it or not)

For most viewers this page is their entire experience of HookBox. Two strings do the acquisition work,
and both are quiet by design:

- **Footer:** *Served by HookBox — self-hosted API mocking and request inspection. This page is
  read-only.*
- **The one link out**, on the unavailable page only: *What is HookBox?* → `/`.

Deliberately **not** on this page: a sign-up CTA, an email field, a "powered by" badge with accent
fill (design.md forbids any accent-filled control here), an OG/unfurl preview (security.md §4.13 —
a link unfurler must not render someone's captured traffic into a chat preview).

### 3.2 Landing deltas — OPTIONAL, outside this PRD's scope

The PRD adds no landing work and no AC covers this; three capabilities in this batch are genuinely
landing-worthy, so the copy exists and the PM can take it or leave it (§7.15). The hero, subhead,
value strip and email gate are **unchanged** — they earn an email, and nothing here helps with that.

- **Read-only share links** — *Send a link, not your secret.* Anyone with the URL can read an
  endpoint's recent requests in a browser — and nothing else. Revoke it when the conversation ends.
- **Take the evidence with you** — Export the visible feed as CSV — headers, bodies, and the response
  HookBox served — straight from the browser.
- **Portable configuration** — Export an endpoint's settings and rules as one JSON file; import them
  into another endpoint, or keep them in the repo next to your code.

Keys in §4.7.

---

## 4. String tables (key → string)

> The FE wires each key 1:1 into `src/lib/copy.ts`. `{…}` = interpolation slot. Strings are final
> copy — a change to a value here is a copy edit, not a code edit. Sections continue the numbering in
> `docs/features/hookbox-rust-replatform/copy.md` (which ends at §5.14).

### 4.1 §5.15 Feed actions & Clear all (`feed.*`) — F1

| Key | String |
|---|---|
| `feed.actions.menu.aria` | Feed actions |
| `feed.actions.emptyHint` | Nothing to clear or export — no requests captured yet. |
| `feed.actions.busyHint` | Finish or cancel the export first. |
| `feed.actions.offlineHint` | You're offline. Reconnect to clear or export. |
| `feed.clearAll` | Clear all |
| `feed.clearAll.aria` | Clear all captured requests |
| `feed.clearAll.confirm.title` | Clear all requests? |
| `feed.clearAll.confirm.body` | Deletes every request captured for "{endpoint}" — not only the ones listed here. The feed starts fresh, and this can't be undone. |
| `feed.clearAll.confirm.note` | Requests that arrive after this show up as normal. |
| `feed.clearAll.confirm.confirm` | Clear all |
| `feed.clearAll.clearing` | Clearing… |
| `feed.clearAll.error` | Couldn't clear the requests. Nothing was deleted. |

**Wiring notes.** `{endpoint}` = `endpoint.name || endpoint.token` (never both, never the mock URL).
No count is passed — see §7.5. `feed.clearAll.confirm.note` is OPTIONAL (§7.17). Success toast reuses
`set.toast.historyCleared`; Cancel reuses `common.cancel`; the in-dialog error slot renders the
server's `detail` when present and falls back to `feed.clearAll.error`.

### 4.2 §5.16 CSV export (`feed.export.*`) — F5

| Key | String |
|---|---|
| `feed.export` | Export CSV |
| `feed.export.aria` | Export the listed requests as CSV |
| `feed.export.note` | Exports the {n} requests listed now, newest first. |
| `feed.export.progress` | Exporting {done} of {total}… |
| `feed.export.progress.aria` | Exporting requests |
| `feed.export.announce` | Exported {done} of {total}. |
| `feed.export.preparing` | Preparing file… |
| `feed.export.cancel` | Cancel |
| `feed.export.cancel.aria` | Cancel the export |
| `feed.export.cancelled` | Export cancelled. No file was downloaded. |
| `feed.export.done` | Exported {n} requests. |
| `feed.export.done.partial` | Exported {n} requests — {m} without detail. |
| `feed.export.error` | Couldn't finish the export. No file was downloaded. |
| `feed.export.error.file` | Couldn't create the file. Your browser may have blocked the download, or the export was too large to build. |
| `feed.export.detailNote` | Rows whose detail couldn't be fetched read pending or unavailable in the four detail columns. |

**Wiring notes.** `feed.export.progress` is both the visible label and the bar's `aria-valuetext`;
`feed.export.preparing` replaces it at 100 % while Cancel stays mounted-but-disabled (design.md §5.3).
`feed.export.announce` is the throttled `sr-only aria-live` string (start / ~25 / 50 / 75 / done
only). `feed.export.detailNote` has no design slot yet — see §7.10.

### 4.3 CSV artifact literals — **FROZEN CONTRACT, not editable copy**

| Key (if wired) | Literal | Frozen by |
|---|---|---|
| `csv.cell.pending` | `pending` | AC-52, §5.6 |
| `csv.cell.unavailable` | `unavailable` | AC-52, §5.6 |
| `csv.header` | `timestamp,method,path,status_code,served_by,duration_ms,request_headers,request_body,response_headers,response_body` | AC-50, §5.6 |

These three go **into the file**, not onto the screen. They are part of a machine-readable format that
downstream consumers parse, so they must never be reworded, capitalised, translated or "improved".
**Recommendation: keep them as constants in `src/lib/csv.ts`, not in `copy.ts`** — listing them here is
so copy review can see every string that leaves the product, not an instruction to make them editable
(§7.13).

### 4.4 §5.17 Configuration export / import (`set.config.*`) — F3

| Key | String |
|---|---|
| `set.config.title` | Configuration |
| `set.config.helper` | Move this endpoint's settings and rules to another endpoint, or keep a copy next to your code. |
| `set.config.export` | Export config |
| `set.config.export.aria` | Download this endpoint's configuration as JSON |
| `set.config.export.busy` | Exporting… |
| `set.config.export.dirty` | Exports the saved configuration — save your changes first to include them. |
| `set.config.export.error` | Couldn't build the export. Nothing was downloaded. |
| `set.config.export.error.rules` | Couldn't read this endpoint's rules, so no file was created. Try again. |
| `set.config.toast.exported` | Configuration exported. |
| `set.config.import` | Import config… |
| `set.config.import.aria` | Choose a HookBox config file to import |
| `set.config.import.helper` | Reads a HookBox config file. Settings are replaced; rules are added to the ones already here. |
| `set.config.import.fileHint` | One .json file, up to 5 MB and 200 rules. |
| `set.config.import.reading` | Checking the file… |
| `set.config.import.invalid.json` | That file isn't valid JSON. If you edited it by hand, check for a missing brace or a trailing comma. |
| `set.config.import.invalid.empty` | That file is empty. |
| `set.config.import.invalid.shape` | That file isn't a HookBox config: {reason} |
| `set.config.import.invalid.field` | {field} is invalid: {reason} |
| `set.config.import.invalid.rule` | Rule {index} is invalid: {reason} |
| `set.config.import.wrongVersion` | Unsupported config version {version}. This build reads version 1. |
| `set.config.import.tooLarge` | That file is larger than 5 MB. |
| `set.config.import.tooManyRules` | That file has {n} rules — the limit is 200. |
| `set.config.import.chooseAnother` | Choose another file |
| `set.config.confirm.title` | Apply this configuration? |
| `set.config.confirm.exported` | Exported {when} from an endpoint named "{name}". |
| `set.config.confirm.exported.unnamed` | Exported {when} from an unnamed endpoint. |
| `set.config.diff.title` | Settings that change ({n}) |
| `set.config.diff.none` | No settings change — the file matches this endpoint. |
| `set.config.diff.unchangedNote` | Settings that stay the same aren't listed. |
| `set.config.diff.change.aria` | {field}: {from} becomes {to} |
| `set.config.diff.empty` | not set |
| `set.config.diff.targetUrl.warning` | target_url changes where unmatched requests are proxied, the moment you apply this. Any active share link starts showing that upstream's responses. |
| `set.config.confirm.rules` | Adds {n} rules to the {existing} already on this endpoint. Nothing in the rules list is replaced or deleted. |
| `set.config.confirm.rules.none` | This file has no rules to add. |
| `set.config.confirm.headerTagWarning` | {n} rules in this file copy request headers into their responses. Anything a caller sends in a header — including credentials — can come back in the body. |
| `set.config.confirm.dirty` | You have unsaved changes on this screen. Applying this discards them. |
| `set.config.confirm.confirm` | Apply configuration |
| `set.config.import.progressConfig` | Applying settings… |
| `set.config.import.progressRules` | Creating rule {i} of {n}… |
| `set.config.import.announce` | Created {i} of {n} rules. |
| `set.config.import.dontClose` | Keep this tab open until it finishes. |
| `set.config.toast.imported` | Configuration imported. |
| `set.config.import.done` | Configuration applied — {n} rules added. |
| `set.config.import.done.noRules` | Configuration applied. This file had no rules. |
| `set.config.import.failedConfig` | Couldn't apply the settings, so no rules were created. The server said: {detail} |
| `set.config.import.failedRule` | Settings were applied and {done} of {total} rules were created. Rule {index} ("{name}") failed: {detail} No rule after it was attempted, and nothing was rolled back. |
| `set.config.import.viewRules` | View rules |

**Wiring notes.**
- `set.config.diff.empty` is the placeholder for a null/empty side of a diff row (e.g. `target_url`
  going from unset to a URL); render it as `text-text-tertiary`, not mono.
- `set.config.import.failedRule`: `{index}` is 1-based (matches "Rule {i} of {n}" progress);
  `{name}` falls back to `rules.row.unnamed` ("Untitled rule") when the bundle's rule has no name;
  `{detail}` is the server's `detail` verbatim, so the string ends its own sentence with a period —
  do not add one.
- `set.config.confirm.headerTagWarning` implements security.md AC-S23 (SHOULD). Unused if the PM
  declines it.
- Dismiss on the persistent failure alert reuses `common.dismiss`; Retry reuses `common.retry`.
- BOM-prefixed files: the FE should **strip a leading U+FEFF before parsing** rather than surfacing a
  parser error — there is no copy for it on purpose (journey.md gap 26).

### 4.5 §5.18 Share links — owner (`share.*`) — F4

Written for **hashed** share codes (architecture D9/D10/D11): the URL exists only in the 201 response,
list rows carry no URL, revoke is by `id`.

| Key | String |
|---|---|
| `share.action` | Share |
| `share.action.aria` | Share this endpoint read-only |
| `share.action.count.aria` | Share this endpoint read-only — {n} active links |
| `share.title` | Share a read-only link |
| `share.intro` | A share link lets anyone holding the URL read this endpoint's recent requests in a browser. No account, no sign-in, and nothing they can change. |
| `share.warning.title` | A share link publishes captured traffic |
| `share.warning.body` | Anyone with the URL can read this endpoint's last 100 requests — including ones that arrived before you created the link, and ones other people sent. Each one shows its method, path, status, headers, query and body, plus the response headers and body HookBox returned. The endpoint's name is visible too. |
| `share.warning.redaction` | Hidden automatically: Authorization, Cookie and X-Owner-Id request headers, and any Set-Cookie or Authorization response header. Nothing else is hidden — don't share an endpoint that carries production secrets. |
| `share.create` | Create share link |
| `share.creating` | Creating… |
| `share.label.label` | Label |
| `share.label.placeholder` | e.g. Acme support ticket #421 |
| `share.label.helper` | Optional, but worth it — the label and the date are all you'll have to tell links apart later. Only you see it. |
| `share.label.tooLong` | Labels are 80 characters or fewer. |
| `share.created.title` | Your share link |
| `share.created.onceHint` | Shown once — copy it now. HookBox keeps only a fingerprint of the link, so it can't show it again. |
| `share.created.lostHint` | If you lose it, revoke the link and create a new one. |
| `share.created.copy.aria` | Copy the share link |
| `share.created.open` | Open in new tab |
| `share.created.open.aria` | Open the share link in a new tab — this is exactly what the recipient sees |
| `share.created.localWarning` | This link points at {origin}, which may not be reachable from outside your network. Set PUBLIC_BASE_URL on the server to mint links other people can open. |
| `share.list.title` | Active links |
| `share.list.count` | {n} of {max} |
| `share.list.hint` | A link's URL is never shown again after it's created — the label and date are how you tell them apart. |
| `share.list.loading.aria` | Loading share links |
| `share.list.empty.title` | No share links yet |
| `share.list.empty.body` | Create one when you need to show someone what their webhook actually sent. |
| `share.list.error.title` | Couldn't load share links |
| `share.list.error.body` | Something went wrong reaching the server. |
| `share.row.untitled` | Untitled link |
| `share.row.created` | Created {when} |
| `share.row.lastUsed` | Opened {when} |
| `share.row.neverUsed` | Never opened |
| `share.row.lastUsed.tooltip` | Recorded at most once a minute, so a very recent open may not show yet. |
| `share.row.revoke` | Revoke |
| `share.row.revoke.aria` | Revoke this share link |
| `share.row.revoke.confirm` | Revoke this link? |
| `share.row.revoke.confirmHint` | It stops working immediately for everyone who has the URL, and it can't be brought back. |
| `share.row.revoke.confirmAction` | Revoke |
| `share.limit.reached` | You have the maximum of {max} active links. Revoke one to create another. |
| `share.toast.created` | Share link created. |
| `share.toast.revoked` | Share link revoked. |
| `share.toast.revokedAlready` | That link was already revoked. |
| `share.error.create` | Couldn't create the link. Try again. |
| `share.error.revoke` | Couldn't revoke the link. It's still active — try again. |
| `share.done` | Done |

**Wiring notes.**
- `share.warning.body` + `share.warning.redaction` are two paragraphs inside one
  `InlineAlert variant="warning"` (title = `share.warning.title`). Both are persistent and sit above
  `Create share link` — this is the mint-time consent moment (security.md AC-S11).
- `share.warning.redaction` asserts the public response-header deny-list from security.md **AC-S1**.
  If AC-S1 is not implemented this string becomes false — §7.1.
- If the PM adopts AC-S12 (window scoped to mint time), swap in the alternate `share.warning.body`
  in §7.2. Do not ship both.
- `share.created.localWarning` is conditional on a client-side reachability heuristic that no AC
  defines — §7.8. It is the only string in the product that names an env var; that's consistent with
  the CLI page naming flags, and it is aimed at the person who runs the server.
- `share.toast.revokedAlready` is the **success** path for a 404 on revoke (journey.md gap 5): the
  row leaves the list and the list refreshes.
- Revoke sends `id`, never the code (architecture D10). No copy anywhere may imply the operator can
  recover, re-copy, or re-send an existing link.

### 4.6 §5.19 Public shared view (`viewer.*`) — F4

Every string on this page is read by someone with no session and no HookBox context.

| Key | String |
|---|---|
| `viewer.docTitle` | Shared requests · HookBox |
| `viewer.title` | Shared requests |
| `viewer.banner.title` | Read-only shared view |
| `viewer.banner.body` | Someone shared one HookBox endpoint's recent requests with you. You're not signed in, and nothing on this page can be changed. |
| `viewer.readOnlyChip` | Read-only |
| `viewer.subject.name` | Endpoint: {name} |
| `viewer.subject.unnamed` | Unnamed endpoint |
| `viewer.subject.since` | Capturing since {when} |
| `viewer.subject.total` | {n} requests received in total |
| `viewer.count` | Showing {n} of the last 100 |
| `viewer.updated` | Updated {when} |
| `viewer.updating` | Shows the last 100 requests from the past 24 hours. Updates every 5 seconds while this tab is open. |
| `viewer.updating.paused` | Paused while this tab is in the background. Switch back to it to resume. |
| `viewer.refresh` | Refresh |
| `viewer.refresh.aria` | Refresh the list |
| `viewer.newRequests.aria` | {n} new requests |
| `viewer.loading.aria` | Loading shared requests |
| `viewer.empty.title` | No requests to show |
| `viewer.empty.body` | This page shows an endpoint's last 100 requests from the past 24 hours. Either nothing has arrived yet, or older requests have already rolled off. It refreshes on its own — you can leave the tab open. |
| `viewer.row.expand.aria` | Show detail for {method} {path}, {status} |
| `viewer.row.collapse.aria` | Hide detail for {method} {path} |
| `viewer.detail.loading.aria` | Loading request detail |
| `viewer.detail.gone.title` | This request is no longer available |
| `viewer.detail.gone.body` | HookBox keeps the last 100 requests for 24 hours. This one has rolled off. The rest of the list still works. |
| `viewer.detail.error.title` | Couldn't load this request |
| `viewer.detail.error.body` | Something went wrong fetching the detail. |
| `viewer.headers.redacted.tooltip` | HookBox hides credential headers in shared views. This value isn't shown. |
| `viewer.unavailable.title` | This link isn't available |
| `viewer.unavailable.body` | It may have been revoked, or it may never have existed. Ask whoever sent it for a new one. |
| `viewer.unavailable.about` | What is HookBox? |
| `viewer.rateLimited.title` | Too many requests |
| `viewer.rateLimited.body` | This page has been loaded too often from your network. It retries in {seconds}s. |
| `viewer.error.title` | Couldn't load the shared requests |
| `viewer.error.body` | Something went wrong reaching the server. Anything already listed may be out of date. |
| `viewer.offline.title` | You're offline |
| `viewer.offline.body` | This page will catch up when your connection returns. |
| `viewer.footer` | Served by HookBox — self-hosted API mocking and request inspection. This page is read-only. |

**Wiring notes.**
- `viewer.title` and `viewer.docTitle` are **static** — never interpolate the endpoint name into
  either (ux.md gap #16). The name appears only via `viewer.subject.name`, truncated, with the full
  value in `title=`.
- `viewer.subject.unnamed` is substituted **into** `viewer.subject.name`'s `{name}` slot (same pattern
  as `rules.row.unnamed`), so the label prefix never disappears.
- `viewer.updating` does double duty (retention window + cadence) so the card needs one caption, not
  two — journey.md gap 13 is covered here rather than in the subject line.
- `viewer.unavailable.*` is **one message for unknown, revoked and tombstoned** (AC-36). Never add a
  variant. No Retry — retrying cannot help.
- `viewer.detail.gone.*` is only correct if a *detail* 404 is non-terminal in the client while a
  *list* 404 is terminal (journey.md BLOCKER 1) — §7.12.
- `viewer.newRequests.aria` is the throttled (≤ once / 10 s) `sr-only aria-live` poll announcement.
- Reused verbatim on this page: `shell.skipLink`, `insp.tab.headers|query|body|response`,
  `insp.headers.empty`, `insp.query.empty`, `insp.body.empty`, `insp.body.binary`,
  `insp.body.pretty|raw`, `insp.response.headers`, `insp.response.body`, `insp.response.empty`,
  `insp.response.servedByLabel`, `insp.headers.redacted`, `common.retry`, `landing.brand.markAlt`,
  `shell.nav.theme.aria`. **Not** reused: `insp.headers.redacted.tooltip` (owner-voiced "your
  secret") → use `viewer.headers.redacted.tooltip`. **Never** rendered here: `servedBy.*.tooltip`
  (all owner-voiced), `feed.empty.*` (leaks the mock URL).

### 4.7 §5.20 Default catch-all rule (`rules.default.*`) — F6

| Key | String |
|---|---|
| `rules.default.add` | Add default rule |
| `rules.default.aria` | Add a catch-all rule that answers any unmatched request |
| `rules.default.helper` | Answers any request no other rule matches — any method, any path — with a 200 and a placeholder JSON body. |
| `rules.default.adding` | Adding… |
| `rules.default.exists` | This endpoint already has an enabled catch-all rule. |
| `rules.default.existsDisabled` | This endpoint already has a catch-all rule, but it's switched off. Turn it back on instead of adding another. |
| `rules.default.toast` | Default catch-all rule added. |
| `rules.default.error` | Couldn't add the default rule. Try again. |
| `rules.default.error.duplicate` | This endpoint already has a catch-all rule. The list has been refreshed. |
| `rules.default.shadow.title` | This will take over unmatched requests |
| `rules.default.shadow.body` | A catch-all rule answers every request no other rule matches, so the fallbacks below stop being reached. |
| `rules.default.shadow.crud` | Auto-CRUD stops serving this endpoint. |
| `rules.default.shadow.tunnel` | Your tunnel stops receiving requests. |
| `rules.default.shadow.proxy` | Requests stop being proxied to your target URL. |
| `rules.default.shadow.echo` | The Echo default response stops being used. |
| `rules.default.shadow.recover` | Switch the rule off or delete it to get them back. |
| `rules.default.shadow.confirm` | Add rule anyway |

**The rule's own content** (sent to the server, then rendered in the rules list and returned to
callers — so it is copy):

| Key | String |
|---|---|
| `rules.default.ruleName` | Catch-all (default) |
| `rules.default.bodyTemplate` | see below |

```json
{
  "ok": true,
  "hookbox": "default catch-all",
  "message": "Edit this rule in HookBox to return your own response."
}
```

As a `copy.ts` value (exact, including newlines and two-space indent):

```ts
'rules.default.bodyTemplate':
  '{\n  "ok": true,\n  "hookbox": "default catch-all",\n  "message": "Edit this rule in HookBox to return your own response."\n}',
```

Why this body: `"ok": true` lets someone wiring a client up get a truthy response immediately (the
whole point of F6), `"hookbox": "default catch-all"` makes it unmistakably a placeholder in a log or
a test failure, and the `message` names the exact next action. It is **not** shaped like a real API
response, so nobody ships against it by accident. This replaces the `body_template` and `name` frozen
in PRD §5.5.7 — §7.4.

**Wiring notes.** `rules.default.shadow.*` renders only when the corresponding fallback is actually
active (`auto_crud`, `tunnel_active`, `target_url`, `default_mode === "echo"`); zero applicable
bullets ⇒ no confirm at all, straight to the POST. `rules.default.existsDisabled` and
`rules.default.error.duplicate` depend on predicate decisions in §7.6/§7.7.

### 4.8 §5.21 Body rendering hints (`insp.*`) — F7

| Key | String |
|---|---|
| `insp.body.largeRaw` | Large body — showing raw text for speed. |
| `insp.body.notJson` | Not valid JSON — showing raw text. |

`insp.body.truncated` already exists and is unwired; see §5 for its changed value.

Both captions render in the owner Inspector **and** in the public viewer's detail well, so both are
deliberately voice-neutral (no "your", no owner context).

### 4.9 Landing deltas (`landing.feature.*`) — OPTIONAL, §7.15

| Key | String |
|---|---|
| `landing.feature.share.title` | Read-only share links |
| `landing.feature.share.body` | Send a link, not your secret. Anyone with the URL can read an endpoint's recent requests in a browser — and nothing else. Revoke it when the conversation ends. |
| `landing.feature.export.title` | Take the evidence with you |
| `landing.feature.export.body` | Export the visible feed as CSV — headers, bodies, and the response HookBox served — straight from the browser. |
| `landing.feature.portable.title` | Portable configuration |
| `landing.feature.portable.body` | Export an endpoint's settings and rules as one JSON file; import them into another endpoint, or keep them in the repo next to your code. |

### 4.10 New shared keys (`common.*`)

| Key | String |
|---|---|
| `common.copy.failed` | Couldn't copy. Select the text and copy it manually. |
| `common.error.endpointGone` | This endpoint no longer exists. Reload the page to catch up. |

**Wiring notes.** `common.copy.failed` requires the `CopyButton` fix in §7.9 — today the component
reports success even when `navigator.clipboard.writeText` throws, which on the shipped port-80 deploy
(a non-secure context) is exactly where an operator copies a **shown-once** share URL.
`common.error.endpointGone` covers the 404/410-mid-session hole for Clear all, Export CSV, Share and
Import (journey.md gap 22).

---

## 5. Changed existing strings

Two shipped values are wrong or incomplete once this batch lands. Both are copy edits to
`src/lib/copy.ts` and to `docs/features/hookbox-rust-replatform/copy.md`.

| Key | Today | Change to | Why |
|---|---|---|---|
| `set.confirm.clearHistory.body` | All {n} traces for this endpoint will be removed. The live feed starts fresh. | Deletes every request captured for this endpoint. The live feed starts fresh, and this can't be undone. | `{n}` is fed from `endpoint.request_count`, a **lifetime** counter, while at most `TRACE_CAP` = 100 traces are stored — so the sentence is wrong for any endpoint past 100 hits, and goes stale the moment F1 clears from the dashboard. Dropping the slot also makes the Settings and dashboard confirms say the same true thing. **FE: remove the `{n}` interpolation at the call site** (ux.md gap #3, journey.md gaps 18/20). |
| `insp.body.truncated` | Truncated at {bytes} — captured bodies are capped. | Capped at {bytes} — this body may be cut short. | AC-70 stores **no** truncation marker, so the only available signal is `len === MAX_BODY_BYTES`, which cannot distinguish "exactly the cap" from "cut at the cap". The old wording asserts a fact we don't have. This is also the key's first wiring (it ships unreferenced today) — ux.md gap #22, journey.md gaps 39/40. |

**Optional change (F2 discoverability, journey.md gap 45):**

| Key | String | Note |
|---|---|---|
| `dash.mockUrl.tooltip` | The endpoint's public mock URL. The local /e/ path is on the Settings screen. | NEW + OPTIONAL. The only proposed replacement for the removed Local path chip's discoverability. `UrlChip` renders no tooltip today, so this needs a small component change; if the PM declines, record the discoverability loss explicitly (R9 accepted the removal, not the loss). |

---

## 6. Reused keys, and ux.md keys I dropped

### 6.1 Reused verbatim — do **not** duplicate

`common.cancel` · `common.retry` · `common.close` · `common.close.aria` · `common.dismiss` ·
`common.copy` · `common.copied` · `common.copy.announce` · `common.error.network` ·
`common.error.generic` · `common.error.401` · `set.confirm.cancel` · `set.toast.historyCleared`
(F1's success toast) · `rules.row.unnamed` (the `{name}` fallback in `set.config.import.failedRule`) ·
`rules.title` · `rules.newRule` · `feed.count` · `feed.empty.*` (owner only) · `insp.tab.*` ·
`insp.headers.empty` · `insp.query.empty` · `insp.body.empty` · `insp.body.binary` ·
`insp.body.pretty` · `insp.body.raw` · `insp.response.*` · `insp.headers.redacted` ·
`shell.skipLink` · `shell.nav.theme.aria` · `landing.brand.markAlt` · `dash.state.gone.*`.

**Ruling on ux.md gap #27** (may owner-authored `insp.*` values appear on an unauthenticated page):
**yes, with one exception.** The tab labels, per-tab empty states and response labels are neutral,
factual and second-person-free — they read the same to a stranger. The one exception is
`insp.headers.redacted.tooltip` ("Sensitive headers (**your secret**, Cookie, Authorization) are never
stored"), which is owner-voiced and, post-AC-S1, incomplete for the response side; the viewer uses
`viewer.headers.redacted.tooltip` instead.

**Pre-existing hardcoded strings that must move to `t()` before they ship publicly** (ux.md §6 item
15): `JsonTree` hardcodes "Pretty", "Raw", "JSON view mode", "(empty)" while `insp.body.pretty` /
`insp.body.raw` sit unused; `KeyValueRows` hardcodes "redacted" and "None" while
`insp.headers.redacted` sits unused. Cheap to fix, and F4 puts all of them on an unauthenticated page,
so AC-64 stops being satisfiable otherwise. `KeyValueRows`'s "None" needs a key —
`insp.headers.none` = **None** — if it is kept.

### 6.2 Keys ux.md §4 proposed that this document drops or renames

| ux.md key | Status | Reason |
|---|---|---|
| `share.created.hint` ("Copy it now — you can always come back to this dialog for it") | **DROPPED** | False under hashed codes. Replaced by `share.created.onceHint` (+ optional `.lostHint`). |
| `share.row.open` / `share.row.open.aria` | **RENAMED** → `share.created.open` / `.open.aria` | With no per-row URL there is nothing to preview from a row; Preview lives in the one-time panel only (architecture D11, design.md §3.6). |
| `share.intro` "…no way to change anything" | reworded | "…and nothing they can change" — same claim, fewer words, no double negative. |
| `set.config.import.invalid` (single generic reason string) | **SPLIT** | AC-16 requires naming the *first failing field/index*; one string can't do that legibly. Now `.invalid.json` / `.empty` / `.shape` / `.field` / `.rule`. |
| `set.config.confirm.body` ("Replaces the nine settings…") | **REPLACED** | security.md AC-S21 requires an actual old→new diff, which makes a prose count of "nine settings" both redundant and wrong when fewer change. Now `set.config.diff.*` + `set.config.confirm.rules`. |
| `feed.export.cancel` = "Cancel export" | reworded → "Cancel" | It sits inside a strip that already names the operation; the object moved to `.cancel.aria`. Confirms design.md §10.6. |
| `viewer.empty.body` ("Nothing has reached this endpoint yet…") | **REPLACED** | Wrong for a link opened after the 100-row cap or 24 h TTL rolled the data off (journey.md E3, gap 13). |
| `viewer.subject.unnamed` = "Unnamed endpoint" | kept, re-scoped | Now substituted into `viewer.subject.name`'s `{name}` slot rather than replacing the whole line. |

---

## 7. Flags for the PM

Copy that touches behaviour the PRD/§5 doesn't yet guarantee, or that changes a frozen value. Items
1–5 should be settled before the FE wires strings.

1. **`share.warning.redaction` asserts security.md AC-S1 (MUST).** ux.md §2.5's disclosure says
   response headers are shown "exactly as they were sent, including any Set-Cookie from your
   upstream" — that becomes **false** if AC-S1's public response-header deny-list ships, and AC-S1 is
   marked MUST. My copy states the deny-list (`Set-Cookie`, `Authorization` → hidden publicly, while
   the owner Inspector and the CSV still show them verbatim per the S-4 ruling). **If AC-S1 is
   dropped, this string must revert to the ux.md wording** — do not ship the deny-list claim without
   the deny-list.
2. **OQ-S1 / AC-S12 changes `share.warning.body`.** If the share window is scoped to
   `created_at >= share.created_at`, use this instead:
   *"Anyone with the URL can read the requests this endpoint receives from now on — up to the last
   100 of them, including ones other people sent. Each one shows its method, path, status, headers,
   query and body, plus the response headers and body HookBox returned. The endpoint's name is
   visible too."*
   Ship exactly one of the two. The un-scoped version (in §4.5) is the one that matches today's
   frozen §5.2 behaviour.
3. **The whole share dialog assumes hashed codes (architecture D9/D10/D11).** `share.created.onceHint`,
   `share.list.hint`, `share.label.helper` and the absence of per-row URLs all depend on it. If R3 is
   re-opened in favour of plaintext (security.md §4 note says **don't**), four strings change and
   `share.row.open*` come back.
4. **`rules.default.bodyTemplate` and `rules.default.ruleName` replace PRD §5.5.7's frozen payload.**
   §5.5.7 currently freezes `name: "Catch-all (default)"` (unchanged) and a `body_template` of
   `{"status": "ok", "message": "Default catch-all response from HookBox. Edit this rule to change
   it."}`. My version is shorter, adds the obviously-placeholder `"hookbox"` key, and names the next
   action. **Needs a §5.5.7 amendment**, plus any AC-58 test fixture asserting the old bytes.
5. **`set.confirm.clearHistory.body` loses its `{n}` slot** (§5). This changes a *shipped* string, so
   any Playwright/visual assertion on it needs updating, and the FE must delete the interpolation.
   Also record the decision that F1's new confirm carries **no** count at all (ux.md gap #3): I did
   not invent a "≤100 stored" number because the client cannot cheaply know the true stored count.
6. **`rules.default.shadow.*` has copy but no AC.** journey.md BLOCKER 36 (a catch-all silently
   disables Auto-CRUD, the tunnel, MITM and Echo) is real and unaddressed in §4.6. I wrote the
   confirm because "every state in journey.md gets words", but if the PM declines it, these eight
   keys are intentionally unreferenced — note them alongside `dash.pathUrl.*` in the AC-10 parity
   check.
7. **`rules.default.existsDisabled` implies a predicate change.** AC-61 keys the disabled state on an
   **enabled** catch-all, so today a *disabled* catch-all lets a second one be created (journey.md
   gap 37). If AC-61 stays as written, this key is unused. `rules.default.error.duplicate` similarly
   assumes the stale-list path is handled (refresh + message) rather than silently duplicating.
8. **`share.created.localWarning` needs a heuristic nobody has specified** (journey.md gap 9,
   required-state "unreachable-origin warning"). Proposal: show it when the minted `url` came back
   origin-relative **and** `window.location.hostname` is `localhost`, `127.0.0.1`, `::1`, `*.local`,
   or an RFC1918 address. It is the only user-facing string that names an env var
   (`PUBLIC_BASE_URL`) — acceptable for a self-hosted single binary, but confirm.
9. **`common.copy.failed` is unreachable without a component fix.** `src/components/ui/copy-button.tsx`
   calls `setCopied(true)` outside its `try/catch`, so it claims success when the Clipboard API is
   unavailable — and the shipped nginx listens on port 80 (non-secure context), i.e. precisely the
   deployment where an operator copies a **shown-once** share URL (journey.md gap 8, HIGH). Needs an
   AC. Until then, `share.created.onceHint` is a promise the copy button can break.
10. **`feed.export.detailNote` has no home.** journey.md gaps 11/34 note that a CSV recipient has no
    way to learn what `pending` / `unavailable` mean. Options: (a) render it as a dismissible
    `InlineAlert variant="info"` in the feed pane after a partial export (needs a design slot);
    (b) fold it into the toast (too long for 3.2 s); (c) drop it and document the sentinels in §5.6
    only. Recommendation: **(a)**, else drop the key. Also flagged: a request body whose literal text
    is `pending` is indistinguishable from the sentinel — a §5.6 documentation problem, not a copy
    one.
11. **`viewer.updating` merges retention window + poll cadence into one caption.** That covers
    journey.md gap 13 in the lightest place. If the PM wants the window higher on the page (in the
    subject line next to "Capturing since"), split it into `viewer.retention` and shorten
    `viewer.updating` back to "Updates every 5 seconds while this tab is open."
12. **`viewer.detail.gone.body` promises "The rest of the list still works."** That is only true if
    the client treats a **detail** 404 as non-terminal while a **list** 404 is terminal (journey.md
    BLOCKER 1, security.md AC-S8). Without that rule the copy lies and the page will render the
    terminal unavailable state instead. Needs the AC.
13. **`csv.*` literals are format, not copy.** Keep `pending`, `unavailable` and the header row as
    constants in `src/lib/csv.ts`; they're listed in §4.3 for review coverage only. If they do land in
    `copy.ts`, they need a comment marking them frozen by AC-50/AC-52 so a future copy pass doesn't
    "improve" them into a broken file format.
14. **Two hardcoded-string cleanups become AC-64 blockers** because F4 puts `JsonTree` and
    `KeyValueRows` on an unauthenticated page (§6.1). One new key may be needed
    (`insp.headers.none` = "None"). Cheap, but it is scope.
15. **§4.9 landing deltas are outside the PRD.** No AC covers them and the PRD's non-goals don't
    mention the landing page. I wrote them because share/export/portability are the three most
    landing-worthy things this batch adds; PM decides whether they ship, and if so whether they go in
    the existing `landing.feature.*` long-page block or nowhere. The hero, subhead, strip and email
    gate are untouched.
16. **`dash.mockUrl.tooltip` (§5) is the only proposed answer to journey.md gap 45.** Accept it, or
    accept the discoverability regression explicitly.
17. **`feed.clearAll.confirm.note` is optional.** It exists to pre-empt the arrival race (journey.md
    gap 21: a `new_request` can land between confirm and 200, so the feed may not be visibly empty).
    Drop it if the dialog reads tighter without it — but then the e2e assertion for AC-4 needs to
    tolerate a non-empty feed.
18. **`share.row.lastUsed.tooltip` is optional** and exists only because architecture coalesces
    `last_used_at` writes to ≥ 60 s. Without it, an operator who opens their own link and sees "Never
    opened" will think the feature is broken.
19. **AC-10 parity check will see more intentionally-unreferenced keys than `dash.pathUrl.*`.**
    Depending on decisions 6, 7, 10, 16, 17 and 18, up to eleven keys may sit in the table unwired.
    Recommend the parity check assert "every key in copy.md exists in copy.ts" (one direction only),
    which is what it does today.

---

## 8. Coverage map — every journey.md state has words

| journey.md required state | Key(s) |
|---|---|
| **Feed header (F1/F5)** loading / empty | `feed.actions.emptyHint` (+ existing `feed.loading.aria`, `feed.empty.*`) |
| success (both enabled) | `feed.clearAll`, `feed.export` |
| confirm | `feed.clearAll.confirm.*` |
| clear in flight | `feed.clearAll.clearing` |
| clear error | `feed.clearAll.error` (+ server `detail` in-dialog) |
| clear success | `set.toast.historyCleared` (reused) |
| exporting | `feed.export.progress`, `feed.export.progress.aria`, `feed.export.announce`, `feed.export.cancel` |
| serialising | `feed.export.preparing` |
| export partial | `feed.export.done.partial` (+ optional `feed.export.detailNote`) |
| export cancelled | `feed.export.cancelled` |
| export failed / no file | `feed.export.error`, `feed.export.error.file` |
| export 401 | existing `common.error.401` (client bounce; no toast) |
| mutually exclusive with clear | `feed.actions.busyHint` |
| offline | `feed.actions.offlineHint` (+ existing `dash.state.offline.*`) |
| endpoint deleted mid-session | `common.error.endpointGone` |
| **Inspector (F7)** response body populated / NULL | existing `insp.response.body` / `insp.response.empty` |
| not JSON | `insp.body.notJson` |
| large body | `insp.body.largeRaw` |
| truncated | `insp.body.truncated` (changed value) |
| **Settings → Configuration (F3)** export idle / busy / error | `set.config.export`, `.export.busy`, `.export.error`, `.export.error.rules` |
| export, form dirty | `set.config.export.dirty` |
| export success | `set.config.toast.exported` |
| import idle | `set.config.import`, `.import.helper`, `.import.fileHint` |
| import validating | `set.config.import.reading` |
| import invalid | `.invalid.json`, `.invalid.empty`, `.invalid.shape`, `.invalid.field`, `.invalid.rule`, `.wrongVersion`, `.tooLarge`, `.tooManyRules`, `.chooseAnother` |
| pre-apply diff + confirm (AC-S21) | `set.config.confirm.*`, `set.config.diff.*` |
| unsaved-edits conflict | `set.config.confirm.dirty` |
| import in flight | `.import.progressConfig`, `.import.progressRules`, `.import.announce`, `.import.dontClose` |
| import success | `set.config.toast.imported`, `.import.done`, `.import.done.noRules` |
| import partial failure | `.import.failedConfig`, `.import.failedRule`, `.import.viewRules` |
| **Share dialog (F4 owner)** loading | `share.list.loading.aria` |
| empty | `share.list.empty.title`, `.empty.body` |
| list | `share.list.title`, `.count`, `.hint`, `share.row.created`, `.lastUsed`, `.neverUsed`, `.untitled` |
| creating | `share.creating` |
| created (shown once) | `share.created.title`, `.onceHint`, `.lostHint`, `.copy.aria`, `.open`, `.open.aria` |
| unreachable origin at mint | `share.created.localWarning` |
| label invalid | `share.label.tooLong` |
| at cap | `share.limit.reached` (+ server 422 `detail`) |
| revoke armed | `share.row.revoke.confirm`, `.confirmHint`, `.confirmAction` |
| revoked | `share.toast.revoked` |
| revoke 404 (already revoked) | `share.toast.revokedAlready` |
| revoke failed | `share.error.revoke` |
| create failed | `share.error.create` |
| list load failed | `share.list.error.title`, `.error.body` (+ `common.retry`) |
| mint-time disclosure | `share.warning.title`, `.body`, `.redaction` |
| **`/s/:code` (F4 viewer)** loading | `viewer.loading.aria` |
| standing orientation | `viewer.banner.title`, `.banner.body`, `viewer.readOnlyChip`, `viewer.footer` |
| empty (valid link, nothing to show) | `viewer.empty.title`, `.empty.body` |
| list | `viewer.count`, `viewer.updated`, `viewer.subject.*`, `viewer.refresh` |
| poll cadence / paused | `viewer.updating`, `viewer.updating.paused` |
| new rows announced | `viewer.newRequests.aria` |
| detail loading / ready | `viewer.detail.loading.aria` (+ reused `insp.*`) |
| detail gone | `viewer.detail.gone.title`, `.gone.body` |
| detail error | `viewer.detail.error.title`, `.error.body` (+ `common.retry`) |
| unavailable (404 — unknown/revoked/deleted) | `viewer.unavailable.title`, `.body`, `.about` |
| rate limited (429) | `viewer.rateLimited.title`, `.body` |
| error (other) | `viewer.error.title`, `.body` |
| offline / hidden tab | `viewer.offline.title`, `.body`, `viewer.updating.paused` |
| hidden credentials | `viewer.headers.redacted.tooltip` |
| **Rules Manager (F6)** add-default idle | `rules.default.add`, `.aria`, `.helper` |
| add-default busy | `rules.default.adding` |
| add-default disabled | `rules.default.exists`, `.existsDisabled` |
| add-default shadowing warning | `rules.default.shadow.*` |
| add-default success | `rules.default.toast` |
| add-default error / duplicate | `rules.default.error`, `.error.duplicate` |
| the rule itself | `rules.default.ruleName`, `rules.default.bodyTemplate` |
| **F2** | no new copy; `dash.pathUrl.*` stay unreferenced (AC-10); optional `dash.mockUrl.tooltip` |

**Namespace audit shortcut:** everything an anonymous visitor can read is under `viewer.*` plus the
explicitly-listed reused keys in §4.6 — so "what can a stranger see?" is one grep for `viewer.` and
one review of that list. That is why the public page does not extend `feed.*` or `insp.*` with new
keys.
