/**
 * RuleBuilder — the 5-tab rule authoring form (PRD §5.3, AC-14, AC-D23/D24,
 * AC-J10, copy.md §5.7). A dialog with five tabs that map 1:1 to the FROZEN
 * §5.3 shapes:
 *
 *   Matching   → MatchCriteria { method, path, headers, query, body_conditions,
 *                                state_requirements }
 *   Response   → ResponseSpec  { status_code, content_type, headers, body_template }
 *   Templating → inserts template tags into body_template (no live render — that
 *                is a documented non-goal); honesty note that unknown tags stay literal
 *   Actions    → state_writes[] + webhook_action (rendered VISIBLE-BUT-DISABLED
 *                with the "Stored, not yet sent" badge so the shape round-trips, AC-J10)
 *   Throttling → per-rule latency_ms / rate_limit_per_min / chaos_mode overrides
 *
 * The body_template byte counter (UTF-8 bytes) turns --warning-fg approaching the
 * 256 KB cap and --danger-fg when exceeded (text + color; AC-D24 / rule.resp.body.counter*).
 * webhook_action is sent (not omitted) so the round-trip is lossless. Save calls
 * #8 createRule or #10 patchRule; strings all come from copy.md rule.* via t().
 */
import { useMemo, useRef, useState } from "react";
import { Plus, Trash2 } from "lucide-react";
import {
  api,
  ApiError,
  type BodyCondition,
  type MockRule,
  type MockRuleCreate,
  type StateRequirement,
  type StateWrite,
} from "@/api";
import { t } from "@/lib/copy";
import { cn } from "@/lib/cn";
import {
  Dialog,
  DialogBody,
  DialogContent,
  DialogFooter,
  DialogHeader,
} from "@/components/ui/dialog";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Button } from "@/components/ui/button";
import { Field, Input, Label, Textarea } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import { Slider } from "@/components/ui/slider";
import { InlineAlert } from "@/components/hookbox/inline-alert";
import { useToast } from "@/components/ui/toast";

const BODY_CAP_BYTES = 256000; // §5.3 / architecture: body_template ≤ 256000 chars/bytes
const BODY_WARN_BYTES = Math.floor(BODY_CAP_BYTES * 0.9);

const METHODS = [
  "ANY",
  "GET",
  "POST",
  "PUT",
  "PATCH",
  "DELETE",
  "HEAD",
  "OPTIONS",
];

/** A mutable working copy of the rule shape (all fields present for the form). */
interface Draft {
  name: string;
  priority: number;
  enabled: boolean;
  method: string;
  path: string;
  headers: [string, string][];
  query: [string, string][];
  body_conditions: BodyCondition[];
  state_requirements: StateRequirement[];
  status_code: number;
  content_type: string;
  resp_headers: [string, string][];
  body_template: string;
  state_writes: StateWrite[];
  webhook_url: string;
  webhook_body: string;
  latency_ms: number | null;
  rate_limit_per_min: number | null;
  chaos_mode: "error" | "dropout" | null;
}

function recordToPairs(r: Record<string, string>): [string, string][] {
  return Object.entries(r);
}
function pairsToRecord(pairs: [string, string][]): Record<string, string> {
  const out: Record<string, string> = {};
  for (const [k, v] of pairs) if (k.trim()) out[k] = v;
  return out;
}

function draftFromRule(rule: MockRule | null): Draft {
  return {
    name: rule?.name ?? "",
    priority: rule?.priority ?? 100,
    enabled: rule?.enabled ?? true,
    method: rule?.match.method ?? "ANY",
    path: rule?.match.path ?? "/*",
    headers: recordToPairs(rule?.match.headers ?? {}),
    query: recordToPairs(rule?.match.query ?? {}),
    body_conditions: rule?.match.body_conditions ?? [],
    state_requirements: rule?.match.state_requirements ?? [],
    status_code: rule?.response.status_code ?? 200,
    content_type: rule?.response.content_type ?? "application/json",
    resp_headers: recordToPairs(rule?.response.headers ?? {}),
    body_template: rule?.response.body_template ?? "",
    state_writes: rule?.state_writes ?? [],
    webhook_url: rule?.webhook_action?.url ?? "",
    webhook_body: rule?.webhook_action?.body_template ?? "",
    latency_ms: rule?.latency_ms ?? null,
    rate_limit_per_min: rule?.rate_limit_per_min ?? null,
    chaos_mode: rule?.chaos_mode ?? null,
  };
}

function draftToPayload(d: Draft): MockRuleCreate {
  return {
    name: d.name.trim() || null,
    priority: d.priority,
    enabled: d.enabled,
    match: {
      method: d.method,
      path: d.path,
      headers: pairsToRecord(d.headers),
      query: pairsToRecord(d.query),
      body_conditions: d.body_conditions.filter((c) => c.path.trim()),
      state_requirements: d.state_requirements.filter((s) => s.key.trim()),
    },
    response: {
      status_code: d.status_code,
      content_type: d.content_type,
      headers: pairsToRecord(d.resp_headers),
      body_template: d.body_template,
    },
    state_writes: d.state_writes.filter((s) => s.key.trim()),
    latency_ms: d.latency_ms,
    rate_limit_per_min: d.rate_limit_per_min,
    chaos_mode: d.chaos_mode,
    // Always send webhook_action when a URL is present so the shape round-trips
    // (AC-J10 — stored, not yet sent). Null otherwise.
    webhook_action: d.webhook_url.trim()
      ? { url: d.webhook_url.trim(), body_template: d.webhook_body }
      : null,
  };
}

const TEMPLATE_TAGS: {
  group: string;
  tags: { key: import("@/lib/copy").CopyKey }[];
}[] = [
  {
    group: "rule.tmpl.group.time",
    tags: [{ key: "rule.tmpl.tag.now.iso" }, { key: "rule.tmpl.tag.now.unix" }],
  },
  {
    group: "rule.tmpl.group.random",
    tags: [
      { key: "rule.tmpl.tag.random.uuid" },
      { key: "rule.tmpl.tag.random.int" },
    ],
  },
  {
    group: "rule.tmpl.group.request",
    tags: [
      { key: "rule.tmpl.tag.request.path" },
      { key: "rule.tmpl.tag.request.query" },
      { key: "rule.tmpl.tag.request.header" },
      { key: "rule.tmpl.tag.request.body" },
    ],
  },
  { group: "rule.tmpl.group.state", tags: [{ key: "rule.tmpl.tag.state" }] },
];

export function RuleBuilder({
  token,
  rule,
  open,
  onOpenChange,
  onSaved,
}: {
  token: string;
  /** null = creating a new rule; a MockRule = editing it. */
  rule: MockRule | null;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onSaved: () => void;
}) {
  const { toast } = useToast();
  const [draft, setDraft] = useState<Draft>(() => draftFromRule(rule));
  const [saving, setSaving] = useState(false);
  const [serverError, setServerError] = useState<string | null>(null);
  const bodyRef = useRef<HTMLTextAreaElement>(null);
  const [tab, setTab] = useState("matching");

  // Re-seed the draft whenever a different rule (or new) opens.
  const seedKey = rule?.id ?? "new";
  const lastSeed = useRef<string | number>(seedKey);
  if (lastSeed.current !== seedKey) {
    lastSeed.current = seedKey;
    setDraft(draftFromRule(rule));
    setServerError(null);
    setTab("matching");
  }

  const set = <K extends keyof Draft>(k: K, v: Draft[K]) =>
    setDraft((d) => ({ ...d, [k]: v }));

  // ── Validation (mirrors §5.3 clamps; the server is authoritative) ──
  const pathError = !draft.path.startsWith("/")
    ? t("rule.error.path.invalid")
    : null;
  const statusError =
    draft.status_code < 100 || draft.status_code > 599
      ? t("rule.error.status.range")
      : null;
  const bodyBytes = useMemo(
    () => new TextEncoder().encode(draft.body_template).length,
    [draft.body_template],
  );
  const bodyOver = bodyBytes > BODY_CAP_BYTES;
  const invalidCount = [
    pathError,
    statusError,
    bodyOver ? "body" : null,
  ].filter(Boolean).length;

  function insertTag(tag: string) {
    const el = bodyRef.current;
    const cur = draft.body_template;
    if (!el) {
      set("body_template", cur + tag);
      return;
    }
    const start = el.selectionStart ?? cur.length;
    const end = el.selectionEnd ?? cur.length;
    const next = cur.slice(0, start) + tag + cur.slice(end);
    set("body_template", next);
    // Restore caret after the inserted tag on the next tick.
    requestAnimationFrame(() => {
      el.focus();
      el.selectionStart = el.selectionEnd = start + tag.length;
    });
  }

  async function save() {
    if (invalidCount > 0) {
      // Jump to the first tab with an error so the user sees it (journey.md).
      if (pathError) setTab("matching");
      else if (statusError || bodyOver) setTab("response");
      return;
    }
    setSaving(true);
    setServerError(null);
    const payload = draftToPayload(draft);
    try {
      if (rule) {
        await api.patchRule(token, rule.id, payload);
        toast(t("rule.toast.updated"));
      } else {
        await api.createRule(token, payload);
        toast(t("rule.toast.saved"));
      }
      onSaved();
      onOpenChange(false);
    } catch (err) {
      if (err instanceof ApiError && err.status === 404 && rule) {
        setServerError(t("rule.error.gone"));
      } else {
        setServerError(t("rule.error.save"));
      }
    } finally {
      setSaving(false);
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="w-[min(720px,94vw)]">
        <DialogHeader>
          {rule ? t("rule.edit.title") : t("rule.new.title")}
        </DialogHeader>
        <DialogBody className="space-y-4 p-0">
          {/* Identity row */}
          <div className="grid grid-cols-1 gap-3 px-6 pt-5 sm:grid-cols-[1fr_8rem_auto]">
            <Field
              label={t("rule.field.name.label")}
              helper={t("rule.field.name.helper")}
              render={(p) => (
                <Input
                  id={p.id}
                  aria-describedby={p.describedBy}
                  placeholder={t("rule.field.name.placeholder")}
                  value={draft.name}
                  onChange={(e) => set("name", e.target.value)}
                />
              )}
            />
            <Field
              label={t("rule.field.priority.label")}
              helper={t("rule.field.priority.helper")}
              render={(p) => (
                <Input
                  id={p.id}
                  aria-describedby={p.describedBy}
                  type="number"
                  value={draft.priority}
                  onChange={(e) => set("priority", Number(e.target.value))}
                />
              )}
            />
            <div className="flex items-end gap-2 pb-1">
              <Label>{t("rule.field.enabled.label")}</Label>
              <Switch
                checked={draft.enabled}
                onCheckedChange={(v) => set("enabled", v)}
                aria-label={t("rule.field.enabled.label")}
              />
            </div>
          </div>

          {serverError && (
            <div className="px-6">
              <InlineAlert variant="danger" role="alert">
                {serverError}
              </InlineAlert>
            </div>
          )}

          <Tabs value={tab} onValueChange={setTab}>
            <TabsList className="px-6">
              <TabsTrigger value="matching">
                {t("rule.tab.matching")}
              </TabsTrigger>
              <TabsTrigger value="response">
                {t("rule.tab.response")}
              </TabsTrigger>
              <TabsTrigger value="templating">
                {t("rule.tab.templating")}
              </TabsTrigger>
              <TabsTrigger value="actions">{t("rule.tab.actions")}</TabsTrigger>
              <TabsTrigger value="throttling">
                {t("rule.tab.throttling")}
              </TabsTrigger>
            </TabsList>

            <div className="px-6 py-4">
              {/* ── Matching ── */}
              <TabsContent value="matching" className="space-y-4">
                <div className="grid grid-cols-1 gap-3 sm:grid-cols-[8rem_1fr]">
                  <Field
                    label={t("rule.match.method.label")}
                    render={(p) => (
                      <select
                        id={p.id}
                        value={draft.method}
                        onChange={(e) => set("method", e.target.value)}
                        className="h-9 w-full rounded-sm border border-border-strong bg-surface px-2 text-body-sm text-text-primary"
                      >
                        {METHODS.map((m) => (
                          <option key={m} value={m}>
                            {m}
                          </option>
                        ))}
                      </select>
                    )}
                  />
                  <Field
                    label={t("rule.match.path.label")}
                    helper={t("rule.match.path.helper")}
                    error={pathError}
                    render={(p) => (
                      <Input
                        id={p.id}
                        aria-describedby={p.describedBy}
                        invalid={p.invalid}
                        mono
                        placeholder={t("rule.match.path.placeholder")}
                        value={draft.path}
                        onChange={(e) => set("path", e.target.value)}
                      />
                    )}
                  />
                </div>

                <PairList
                  label={t("rule.match.headers.label")}
                  helper={t("rule.match.headers.helper")}
                  keyPlaceholder={t("rule.match.headers.keyPlaceholder")}
                  valPlaceholder={t("rule.match.headers.valPlaceholder")}
                  pairs={draft.headers}
                  onChange={(p) => set("headers", p)}
                />
                <PairList
                  label={t("rule.match.query.label")}
                  keyPlaceholder={t("rule.match.query.keyPlaceholder")}
                  valPlaceholder={t("rule.match.query.valPlaceholder")}
                  pairs={draft.query}
                  onChange={(p) => set("query", p)}
                />

                <BodyConditions
                  conditions={draft.body_conditions}
                  onChange={(c) => set("body_conditions", c)}
                />
                <StateRequirements
                  reqs={draft.state_requirements}
                  onChange={(s) => set("state_requirements", s)}
                />
              </TabsContent>

              {/* ── Response ── */}
              <TabsContent value="response" className="space-y-4">
                <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
                  <Field
                    label={t("rule.resp.status.label")}
                    helper={t("rule.resp.status.helper")}
                    error={statusError}
                    render={(p) => (
                      <Input
                        id={p.id}
                        aria-describedby={p.describedBy}
                        invalid={p.invalid}
                        type="number"
                        value={draft.status_code}
                        onChange={(e) =>
                          set("status_code", Number(e.target.value))
                        }
                      />
                    )}
                  />
                  <Field
                    label={t("rule.resp.contentType.label")}
                    render={(p) => (
                      <Input
                        id={p.id}
                        mono
                        value={draft.content_type}
                        onChange={(e) => set("content_type", e.target.value)}
                      />
                    )}
                  />
                </div>

                <PairList
                  label={t("rule.resp.headers.label")}
                  keyPlaceholder={t("rule.match.headers.keyPlaceholder")}
                  valPlaceholder={t("rule.match.headers.valPlaceholder")}
                  pairs={draft.resp_headers}
                  onChange={(p) => set("resp_headers", p)}
                />

                <div className="space-y-1.5">
                  <div className="flex items-center justify-between">
                    <Label htmlFor="rule-body">
                      {t("rule.resp.body.label")}
                    </Label>
                    <span
                      className={cn(
                        "text-caption tnum",
                        bodyOver
                          ? "text-danger-fg"
                          : bodyBytes >= BODY_WARN_BYTES
                            ? "text-warning-fg"
                            : "text-text-tertiary",
                      )}
                      aria-live="polite"
                    >
                      {bodyOver
                        ? t("rule.resp.body.counter.over", {
                            used: formatKb(bodyBytes),
                          })
                        : t("rule.resp.body.counter", {
                            used: formatKb(bodyBytes),
                          })}
                    </span>
                  </div>
                  <Textarea
                    id="rule-body"
                    ref={bodyRef}
                    mono
                    rows={8}
                    invalid={bodyOver}
                    placeholder={t("rule.resp.body.placeholder")}
                    value={draft.body_template}
                    onChange={(e) => set("body_template", e.target.value)}
                  />
                  {bodyOver && (
                    <p className="text-body-sm text-danger-fg" role="alert">
                      {t("rule.error.body.tooLarge")}
                    </p>
                  )}
                </div>
              </TabsContent>

              {/* ── Templating (inserts tags; no live render — non-goal) ── */}
              <TabsContent value="templating" className="space-y-4">
                <div className="space-y-1">
                  <p className="text-body-sm text-text-secondary">
                    {t("rule.tmpl.intro")}
                  </p>
                  <p className="text-caption text-text-tertiary">
                    {t("rule.tmpl.honesty")}
                  </p>
                </div>
                {TEMPLATE_TAGS.map((g) => (
                  <div key={g.group} className="space-y-1.5">
                    <Label>{t(g.group as import("@/lib/copy").CopyKey)}</Label>
                    <div className="flex flex-wrap gap-2">
                      {g.tags.map(({ key }) => {
                        const tag = t(key);
                        return (
                          <button
                            key={key}
                            type="button"
                            onClick={() => insertTag(tag)}
                            className="rounded-xs border border-border bg-subtle px-2 py-1 font-mono text-mono-sm text-text-primary hover:bg-hover"
                          >
                            {tag}
                          </button>
                        );
                      })}
                    </div>
                  </div>
                ))}
              </TabsContent>

              {/* ── Actions (state_writes + webhook visible-but-disabled) ── */}
              <TabsContent value="actions" className="space-y-5">
                <StateWrites
                  writes={draft.state_writes}
                  onChange={(s) => set("state_writes", s)}
                />

                {/* Webhook — stored, not yet sent (AC-J10). Visible but disabled. */}
                <div className="space-y-2">
                  <div className="flex items-center gap-2">
                    <Label>{t("rule.act.webhook.label")}</Label>
                    <span className="inline-flex rounded-xs bg-warning-bg px-1.5 py-0.5 text-caption font-medium text-warning-fg">
                      {t("rule.act.webhook.badge")}
                    </span>
                  </div>
                  <p className="text-body-sm text-text-tertiary">
                    {t("rule.act.webhook.helper")}
                  </p>
                  <Input
                    mono
                    placeholder={t("rule.act.webhook.urlPlaceholder")}
                    value={draft.webhook_url}
                    onChange={(e) => set("webhook_url", e.target.value)}
                  />
                  <Textarea
                    mono
                    rows={3}
                    placeholder={t("rule.act.webhook.bodyPlaceholder")}
                    value={draft.webhook_body}
                    onChange={(e) => set("webhook_body", e.target.value)}
                  />
                </div>
              </TabsContent>

              {/* ── Throttling (per-rule overrides) ── */}
              <TabsContent value="throttling" className="space-y-5">
                <p className="text-body-sm text-text-secondary">
                  {t("rule.thr.intro")}
                </p>
                <NumberWithSlider
                  label={t("rule.thr.latency.label")}
                  unit={t("rule.thr.latency.unit")}
                  helper={t("rule.thr.latency.helper")}
                  min={0}
                  max={10000}
                  value={draft.latency_ms ?? 0}
                  onChange={(v) => set("latency_ms", v)}
                />
                <NumberWithSlider
                  label={t("rule.thr.rateLimit.label")}
                  unit={t("rule.thr.rateLimit.unit")}
                  helper={t("rule.thr.rateLimit.helper")}
                  min={0}
                  max={6000}
                  value={draft.rate_limit_per_min ?? 0}
                  onChange={(v) => set("rate_limit_per_min", v)}
                />
                <Field
                  label={t("rule.thr.chaosMode.label")}
                  render={(p) => (
                    <select
                      id={p.id}
                      value={draft.chaos_mode ?? ""}
                      onChange={(e) =>
                        set(
                          "chaos_mode",
                          e.target.value === ""
                            ? null
                            : (e.target.value as "error" | "dropout"),
                        )
                      }
                      className="h-9 w-full rounded-sm border border-border-strong bg-surface px-2 text-body-sm text-text-primary"
                    >
                      <option value="">
                        {t("rule.thr.chaosMode.inherit")}
                      </option>
                      <option value="error">
                        {t("rule.thr.chaosMode.error")}
                      </option>
                      <option value="dropout">
                        {t("rule.thr.chaosMode.dropout")}
                      </option>
                    </select>
                  )}
                />
              </TabsContent>
            </div>
          </Tabs>
        </DialogBody>
        <DialogFooter>
          <span
            className="mr-auto text-caption text-text-tertiary"
            aria-live="polite"
          >
            {invalidCount > 0
              ? t("rule.footer.invalid", { n: invalidCount })
              : t("rule.footer.ready")}
          </span>
          <Button variant="ghost" onClick={() => onOpenChange(false)}>
            {t("rule.cancel")}
          </Button>
          <Button
            variant="primary"
            onClick={save}
            loading={saving}
            disabled={invalidCount > 0}
          >
            {saving ? t("rule.saving") : t("rule.save")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function formatKb(bytes: number): string {
  return (bytes / 1024).toFixed(1);
}

// ── Reusable row editors ──

function PairList({
  label,
  helper,
  keyPlaceholder,
  valPlaceholder,
  pairs,
  onChange,
}: {
  label: string;
  helper?: string;
  keyPlaceholder: string;
  valPlaceholder: string;
  pairs: [string, string][];
  onChange: (pairs: [string, string][]) => void;
}) {
  return (
    <fieldset className="space-y-1.5">
      <Label>{label}</Label>
      {helper && <p className="text-body-sm text-text-tertiary">{helper}</p>}
      <div className="space-y-2">
        {pairs.map((pair, i) => (
          <div key={i} className="flex items-center gap-2">
            <Input
              mono
              placeholder={keyPlaceholder}
              value={pair[0]}
              onChange={(e) =>
                onChange(
                  pairs.map((p, j) => (j === i ? [e.target.value, p[1]] : p)),
                )
              }
            />
            <Input
              mono
              placeholder={valPlaceholder}
              value={pair[1]}
              onChange={(e) =>
                onChange(
                  pairs.map((p, j) => (j === i ? [p[0], e.target.value] : p)),
                )
              }
            />
            <RemoveButton
              onClick={() => onChange(pairs.filter((_, j) => j !== i))}
            />
          </div>
        ))}
      </div>
      <AddButton onClick={() => onChange([...pairs, ["", ""]])} />
    </fieldset>
  );
}

function BodyConditions({
  conditions,
  onChange,
}: {
  conditions: BodyCondition[];
  onChange: (c: BodyCondition[]) => void;
}) {
  const ops: { v: BodyCondition["op"]; label: string }[] = [
    { v: "eq", label: t("rule.match.body.op.eq") },
    { v: "neq", label: t("rule.match.body.op.neq") },
    { v: "contains", label: t("rule.match.body.op.contains") },
    { v: "exists", label: t("rule.match.body.op.exists") },
  ];
  return (
    <fieldset className="space-y-1.5">
      <Label>{t("rule.match.body.label")}</Label>
      <p className="text-body-sm text-text-tertiary">
        {t("rule.match.body.helper")}
      </p>
      <div className="space-y-2">
        {conditions.map((c, i) => (
          <div key={i} className="flex items-center gap-2">
            <Input
              mono
              placeholder={t("rule.match.body.pathPlaceholder")}
              value={c.path}
              onChange={(e) =>
                onChange(
                  conditions.map((x, j) =>
                    j === i ? { ...x, path: e.target.value } : x,
                  ),
                )
              }
            />
            <select
              value={c.op}
              onChange={(e) =>
                onChange(
                  conditions.map((x, j) =>
                    j === i
                      ? { ...x, op: e.target.value as BodyCondition["op"] }
                      : x,
                  ),
                )
              }
              className="h-9 rounded-sm border border-border-strong bg-surface px-2 text-body-sm text-text-primary"
            >
              {ops.map((o) => (
                <option key={o.v} value={o.v}>
                  {o.label}
                </option>
              ))}
            </select>
            <Input
              mono
              disabled={c.op === "exists"}
              placeholder={t("rule.match.body.valPlaceholder")}
              value={c.value ?? ""}
              onChange={(e) =>
                onChange(
                  conditions.map((x, j) =>
                    j === i ? { ...x, value: e.target.value } : x,
                  ),
                )
              }
            />
            <RemoveButton
              onClick={() => onChange(conditions.filter((_, j) => j !== i))}
            />
          </div>
        ))}
      </div>
      <AddButton
        onClick={() =>
          onChange([...conditions, { path: "", op: "eq", value: "" }])
        }
      />
    </fieldset>
  );
}

function StateRequirements({
  reqs,
  onChange,
}: {
  reqs: StateRequirement[];
  onChange: (s: StateRequirement[]) => void;
}) {
  const ops: { v: StateRequirement["op"]; label: string }[] = [
    { v: "eq", label: t("rule.match.state.op.eq") },
    { v: "neq", label: t("rule.match.state.op.neq") },
    { v: "exists", label: t("rule.match.state.op.exists") },
    { v: "absent", label: t("rule.match.state.op.absent") },
  ];
  return (
    <fieldset className="space-y-1.5">
      <Label>{t("rule.match.state.label")}</Label>
      <p className="text-body-sm text-text-tertiary">
        {t("rule.match.state.helper")}
      </p>
      <div className="space-y-2">
        {reqs.map((s, i) => (
          <div key={i} className="flex items-center gap-2">
            <Input
              mono
              placeholder={t("rule.match.state.keyPlaceholder")}
              value={s.key}
              onChange={(e) =>
                onChange(
                  reqs.map((x, j) =>
                    j === i ? { ...x, key: e.target.value } : x,
                  ),
                )
              }
            />
            <select
              value={s.op}
              onChange={(e) =>
                onChange(
                  reqs.map((x, j) =>
                    j === i
                      ? { ...x, op: e.target.value as StateRequirement["op"] }
                      : x,
                  ),
                )
              }
              className="h-9 rounded-sm border border-border-strong bg-surface px-2 text-body-sm text-text-primary"
            >
              {ops.map((o) => (
                <option key={o.v} value={o.v}>
                  {o.label}
                </option>
              ))}
            </select>
            <Input
              mono
              disabled={s.op === "exists" || s.op === "absent"}
              placeholder={t("rule.match.state.valPlaceholder")}
              value={s.value ?? ""}
              onChange={(e) =>
                onChange(
                  reqs.map((x, j) =>
                    j === i ? { ...x, value: e.target.value } : x,
                  ),
                )
              }
            />
            <RemoveButton
              onClick={() => onChange(reqs.filter((_, j) => j !== i))}
            />
          </div>
        ))}
      </div>
      <AddButton
        onClick={() => onChange([...reqs, { key: "", op: "eq", value: "" }])}
      />
    </fieldset>
  );
}

function StateWrites({
  writes,
  onChange,
}: {
  writes: StateWrite[];
  onChange: (s: StateWrite[]) => void;
}) {
  return (
    <fieldset className="space-y-1.5">
      <Label>{t("rule.act.stateWrites.label")}</Label>
      <p className="text-body-sm text-text-tertiary">
        {t("rule.act.stateWrites.helper")}
      </p>
      <div className="space-y-2">
        {writes.map((w, i) => (
          <div key={i} className="flex items-center gap-2">
            <Input
              mono
              placeholder={t("rule.act.stateWrites.keyPlaceholder")}
              value={w.key}
              onChange={(e) =>
                onChange(
                  writes.map((x, j) =>
                    j === i ? { ...x, key: e.target.value } : x,
                  ),
                )
              }
            />
            <Input
              mono
              placeholder={t("rule.act.stateWrites.valPlaceholder")}
              value={w.value}
              onChange={(e) =>
                onChange(
                  writes.map((x, j) =>
                    j === i ? { ...x, value: e.target.value } : x,
                  ),
                )
              }
            />
            <RemoveButton
              onClick={() => onChange(writes.filter((_, j) => j !== i))}
            />
          </div>
        ))}
      </div>
      <AddButton
        onClick={() => onChange([...writes, { key: "", value: "" }])}
      />
    </fieldset>
  );
}

function NumberWithSlider({
  label,
  unit,
  helper,
  min,
  max,
  value,
  onChange,
}: {
  label: string;
  unit: string;
  helper: string;
  min: number;
  max: number;
  value: number;
  onChange: (v: number) => void;
}) {
  const clamp = (v: number) => Math.min(max, Math.max(min, v));
  return (
    <div className="space-y-1.5">
      <Label>{label}</Label>
      <div className="flex items-center gap-3">
        <Slider
          min={min}
          max={max}
          value={clamp(value)}
          onChange={onChange}
          aria-label={label}
        />
        <div className="flex items-center gap-1">
          <Input
            type="number"
            className="w-24"
            value={value}
            onChange={(e) => onChange(clamp(Number(e.target.value)))}
          />
          <span className="text-caption text-text-tertiary">{unit}</span>
        </div>
      </div>
      <p className="text-body-sm text-text-tertiary">{helper}</p>
    </div>
  );
}

function AddButton({ onClick }: { onClick: () => void }) {
  return (
    <Button type="button" variant="ghost" size="sm" onClick={onClick}>
      <Plus className="h-3.5 w-3.5" aria-hidden="true" />
      {t("rule.match.addRow")}
    </Button>
  );
}

function RemoveButton({ onClick }: { onClick: () => void }) {
  return (
    <Button
      type="button"
      variant="ghost"
      size="icon-sm"
      onClick={onClick}
      aria-label={t("rule.match.removeRow.aria")}
    >
      <Trash2 className="h-3.5 w-3.5" aria-hidden="true" />
    </Button>
  );
}
