/**
 * RulesManager (/d/:token/rules) — lists an endpoint's rules and drives the
 * RuleBuilder for create/edit/delete/duplicate (PRD §5.2 #7–#11, AC-14,
 * copy.md §5.8). Rules render in server order (ORDER BY priority, id — #7 is
 * authoritative; we do NOT re-sort client-side). States: loading · empty (the
 * resolution-order honesty note) · error+Retry · list. Per-row menu:
 * Edit · Duplicate · Delete (typed-confirm dialog). The enable/disable toggle
 * does an optimistic PATCH (#10) with revert-on-error.
 *
 * Opening with ?new=1 (the dashboard "New rule" link) auto-opens the builder.
 * Wrapped in the AppShell so the dashboard chrome stays consistent.
 */
import { useCallback, useEffect, useState } from "react";
import {
  Link,
  useNavigate,
  useParams,
  useSearchParams,
} from "react-router-dom";
import { ArrowLeft, MoreHorizontal, Plus, RotateCw } from "lucide-react";
import {
  api,
  ApiError,
  mockRuleSchema,
  session,
  type EndpointDetail,
  type EndpointSummary,
  type MockRule,
  type MockRuleCreate,
} from "@/api";
import { z } from "zod";
import { t } from "@/lib/copy";
import { AppShell } from "@/components/hookbox/app-shell";
import { ConfirmDialog } from "@/components/hookbox/confirm-dialog";
import { MethodBadge } from "@/components/hookbox/method-badge";
import { InlineAlert } from "@/components/hookbox/inline-alert";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import { SkeletonLines } from "@/components/ui/skeleton";
import { Menu, MenuContent, MenuItem, MenuTrigger } from "@/components/ui/menu";
import { Tooltip } from "@/components/ui/tooltip";
import {
  Dialog,
  DialogBody,
  DialogContent,
  DialogFooter,
  DialogHeader,
} from "@/components/ui/dialog";
import { useToast } from "@/components/ui/toast";
import { RuleBuilder } from "./rule-builder";

/** The parsed §5.3 MockRule. The client validates with mockRuleSchema at
 * runtime; we re-assert the parsed (output) type here because the client's
 * generic infers the looser input type for schemas carrying zod defaults. */
type RuleRow = MockRule;

type ListState =
  | { kind: "loading" }
  | { kind: "error" }
  | { kind: "ready"; rules: RuleRow[] };

/** The frozen §5.5.7 payload (as amended by copy.md/AC-125) — the exact
 * `MockRuleCreate` body for F6's "Add default rule". `name` and
 * `response.body_template` come from copy.ts so a copy edit never drifts
 * from the request the client actually sends (AC-58/AC-59). */
export const DEFAULT_CATCH_ALL_RULE: MockRuleCreate = {
  name: t("rules.default.ruleName"),
  priority: 1000,
  enabled: true,
  match: {
    method: "ANY",
    path: "/*",
    headers: {},
    query: {},
    body_conditions: [],
    state_requirements: [],
  },
  response: {
    status_code: 200,
    headers: {},
    content_type: "application/json",
    body_template: t("rules.default.bodyTemplate"),
  },
  state_writes: [],
  latency_ms: null,
  rate_limit_per_min: null,
  chaos_mode: null,
  webhook_action: null,
};

/** AC-61: a catch-all is `method === "ANY" && path === "/*"` — no `enabled`
 * filter, so a DISABLED catch-all also blocks a second one. */
function hasCatchAllRule(rules: readonly RuleRow[]): boolean {
  return rules.some((r) => r.match.method === "ANY" && r.match.path === "/*");
}

/** AC-122: the four fallbacks a catch-all silently shadows, per the engine's
 * matched-rule short-circuit (backend/src/interceptor/engine.rs:141-145 vs
 * :228-245's resolve_unmatched). Each renders as one bullet, and only when
 * actually active — never a paragraph listing switched-off fallbacks. */
function activeShadowedFallbacks(endpoint: EndpointDetail | null): string[] {
  if (!endpoint) return [];
  const bullets: string[] = [];
  if (endpoint.auto_crud) bullets.push(t("rules.default.shadow.crud"));
  if (endpoint.tunnel_active) bullets.push(t("rules.default.shadow.tunnel"));
  if (endpoint.target_url) bullets.push(t("rules.default.shadow.proxy"));
  if (endpoint.default_mode === "echo")
    bullets.push(t("rules.default.shadow.echo"));
  return bullets;
}

export function RulesManager() {
  const { token = "" } = useParams();
  const navigate = useNavigate();
  const { toast } = useToast();
  const [search, setSearch] = useSearchParams();
  const hasSession = !!session.getSecret();

  const [state, setState] = useState<ListState>({ kind: "loading" });
  const [endpoint, setEndpoint] = useState<EndpointDetail | null>(null);
  const [endpoints, setEndpoints] = useState<EndpointSummary[]>([]);

  // Builder + delete dialog state.
  const [builderOpen, setBuilderOpen] = useState(false);
  const [editing, setEditing] = useState<RuleRow | null>(null);
  const [deleting, setDeleting] = useState<RuleRow | null>(null);

  // F6 "Add default rule" state.
  const [addingDefault, setAddingDefault] = useState(false);
  const [shadowConfirmOpen, setShadowConfirmOpen] = useState(false);

  const load = useCallback(async () => {
    setState({ kind: "loading" });
    try {
      // Re-parse to the precise output type (client guarantees valid shape).
      const rules = z.array(mockRuleSchema).parse(await api.listRules(token));
      setState({ kind: "ready", rules });
    } catch (err) {
      if (err instanceof ApiError && err.status === 401) return; // bounced
      setState({ kind: "error" });
    }
  }, [token]);

  useEffect(() => {
    if (!hasSession) return;
    void load();
    api
      .getEndpoint(token)
      .then(setEndpoint)
      .catch(() => {});
    api
      .listEndpoints()
      .then(setEndpoints)
      .catch(() => {});
  }, [hasSession, token, load]);

  // ?new=1 auto-opens the builder for a fresh rule, then strips the param.
  useEffect(() => {
    if (search.get("new") === "1") {
      setEditing(null);
      setBuilderOpen(true);
      search.delete("new");
      setSearch(search, { replace: true });
    }
  }, [search, setSearch]);

  if (!hasSession) {
    navigate("/", { replace: true, state: { reason: t("common.error.401") } });
    return null;
  }

  function openNew() {
    setEditing(null);
    setBuilderOpen(true);
  }
  function openEdit(rule: RuleRow) {
    setEditing(rule);
    setBuilderOpen(true);
  }

  async function duplicate(rule: RuleRow) {
    try {
      await api.createRule(token, {
        name: rule.name ? `${rule.name} (copy)` : null,
        priority: rule.priority,
        enabled: rule.enabled,
        match: rule.match,
        response: rule.response,
        state_writes: rule.state_writes,
        latency_ms: rule.latency_ms,
        rate_limit_per_min: rule.rate_limit_per_min,
        chaos_mode: rule.chaos_mode,
        webhook_action: rule.webhook_action,
      });
      toast(t("rules.toast.duplicated"));
      void load();
    } catch {
      toast(t("rule.error.save"), "danger");
    }
  }

  async function toggleEnabled(rule: RuleRow, enabled: boolean) {
    if (state.kind !== "ready") return;
    // Optimistic update; revert on error (rules.error.toggle).
    const prev = state.rules;
    setState({
      kind: "ready",
      rules: prev.map((r) => (r.id === rule.id ? { ...r, enabled } : r)),
    });
    try {
      await api.patchRule(token, rule.id, { enabled });
      toast(enabled ? t("rules.toast.enabled") : t("rules.toast.disabled"));
    } catch {
      setState({ kind: "ready", rules: prev });
      toast(t("rules.error.toggle"), "danger");
    }
  }

  async function confirmDelete() {
    if (!deleting) return;
    try {
      await api.deleteRule(token, deleting.id);
      toast(t("rules.toast.deleted"));
      setDeleting(null);
      void load();
    } catch {
      toast(t("rule.error.save"), "danger");
    }
  }

  // F6 — the actual POST, shared by the direct (zero-fallback) path and the
  // shadow confirm's "Add rule anyway". Re-checks the list immediately before
  // creating (AC-123c): another tab may have added a catch-all since our last
  // load, and there is no server-side uniqueness guard.
  async function createDefaultRule() {
    const fresh = z.array(mockRuleSchema).parse(await api.listRules(token));
    setState({ kind: "ready", rules: fresh });
    if (hasCatchAllRule(fresh)) {
      toast(t("rules.default.error.duplicate"), "danger");
      return;
    }
    await api.createRule(token, DEFAULT_CATCH_ALL_RULE);
    toast(t("rules.default.toast"));
    void load();
  }

  async function handleAddDefaultDirect() {
    setAddingDefault(true);
    try {
      await createDefaultRule();
    } catch {
      toast(t("rules.default.error"), "danger");
    } finally {
      setAddingDefault(false);
    }
  }

  // Used as the shadow confirm's onConfirm — FE-0's ConfirmDialog renders any
  // rejection inline, so this deliberately does NOT catch.
  async function handleAddDefaultConfirmed() {
    setAddingDefault(true);
    try {
      await createDefaultRule();
    } finally {
      setAddingDefault(false);
    }
  }

  const existingCatchAll = state.kind === "ready" ? state.rules.find(
    (r) => r.match.method === "ANY" && r.match.path === "/*",
  ) : undefined;
  const addDefaultDisabled =
    !!existingCatchAll || !endpoint || addingDefault || state.kind !== "ready";
  const addDefaultReason = existingCatchAll
    ? existingCatchAll.enabled
      ? t("rules.default.exists")
      : t("rules.default.existsDisabled")
    : null;
  const shadowBullets = activeShadowedFallbacks(endpoint);

  function handleAddDefaultClick() {
    if (shadowBullets.length > 0) {
      setShadowConfirmOpen(true);
    } else {
      void handleAddDefaultDirect();
    }
  }

  return (
    <AppShell token={token} endpoint={endpoint} endpoints={endpoints}>
      <div className="mx-auto h-full max-w-3xl overflow-auto p-6">
        <div className="mb-4 flex items-center justify-between">
          <div className="flex items-center gap-3">
            <Button
              variant="ghost"
              size="sm"
              asChild
              aria-label={t("dash.action.rules")}
            >
              <Link to={`/d/${token}`}>
                <ArrowLeft className="h-4 w-4" aria-hidden="true" />
              </Link>
            </Button>
            <h1 className="text-h2 text-text-primary">{t("rules.title")}</h1>
          </div>
          <div className="flex items-center gap-2">
            <AddDefaultRuleButton
              disabled={addDefaultDisabled}
              reason={addDefaultReason}
              loading={addingDefault}
              onClick={handleAddDefaultClick}
            />
            <Button variant="primary" size="sm" onClick={openNew}>
              <Plus className="h-4 w-4" aria-hidden="true" />
              {t("rules.newRule")}
            </Button>
          </div>
        </div>

        {state.kind === "loading" && (
          <div aria-busy="true">
            <span className="sr-only">{t("rules.loading.aria")}</span>
            <SkeletonLines lines={6} />
          </div>
        )}

        {state.kind === "error" && (
          <InlineAlert
            variant="danger"
            role="alert"
            title={t("rules.error.title")}
            action={
              <Button variant="secondary" size="sm" onClick={() => void load()}>
                <RotateCw className="h-3.5 w-3.5" aria-hidden="true" />
                {t("rules.error.retry")}
              </Button>
            }
          />
        )}

        {state.kind === "ready" && state.rules.length === 0 && (
          <div className="rounded-md border border-border bg-surface p-8 text-center">
            <h2 className="text-h4 text-text-primary">
              {t("rules.empty.title")}
            </h2>
            <p className="mx-auto mt-2 max-w-md text-body-sm text-text-tertiary">
              {t("rules.empty.body")}
            </p>
            <AddDefaultRuleButton
              className="mt-4"
              disabled={addDefaultDisabled}
              reason={addDefaultReason}
              loading={addingDefault}
              onClick={handleAddDefaultClick}
            />
            <p className="mx-auto mt-2 max-w-md text-caption text-text-tertiary">
              {t("rules.default.helper")}
            </p>
          </div>
        )}

        {state.kind === "ready" && state.rules.length > 0 && (
          <div className="overflow-hidden rounded-md border border-border bg-surface">
            {/* Column header — aligns with each row's grid (AC-D18). */}
            <div className="grid grid-cols-[4rem_1fr_auto_3rem_2rem] items-center gap-2 border-b border-border bg-surface-subtle px-3 py-2 text-overline uppercase tracking-wide text-text-tertiary">
              <span>{t("rules.col.priority")}</span>
              <span>{t("rules.col.name")}</span>
              <span>{t("rules.col.match")}</span>
              <span className="text-center">{t("rules.col.enabled")}</span>
              <span className="sr-only">{t("rules.row.menu.aria")}</span>
            </div>
            {state.rules.map((rule) => (
              <div
                key={rule.id}
                className="grid grid-cols-[4rem_1fr_auto_3rem_2rem] items-center gap-2 border-b border-border px-3 py-2.5 last:border-b-0"
              >
                <span className="tnum font-mono text-mono-sm text-text-secondary">
                  {rule.priority}
                </span>
                <button
                  type="button"
                  onClick={() => openEdit(rule)}
                  className="truncate text-left text-body-sm text-text-primary hover:underline"
                >
                  {rule.name || t("rules.row.unnamed")}
                </button>
                <span className="flex items-center gap-2">
                  <MethodBadge method={rule.match.method} />
                  <code
                    className="truncate font-mono text-mono-sm text-text-secondary"
                    title={rule.match.path}
                  >
                    {rule.match.path}
                  </code>
                </span>
                <span className="flex justify-center">
                  <Switch
                    checked={rule.enabled}
                    onCheckedChange={(v) => void toggleEnabled(rule, v)}
                    aria-label={
                      rule.enabled
                        ? t("rules.toast.disabled")
                        : t("rules.toast.enabled")
                    }
                  />
                </span>
                <Menu>
                  <MenuTrigger asChild>
                    <Button
                      variant="ghost"
                      size="icon-sm"
                      aria-label={t("rules.row.menu.aria")}
                    >
                      <MoreHorizontal className="h-4 w-4" aria-hidden="true" />
                    </Button>
                  </MenuTrigger>
                  <MenuContent align="end">
                    <MenuItem onSelect={() => openEdit(rule)}>
                      {t("rules.row.edit")}
                    </MenuItem>
                    <MenuItem onSelect={() => void duplicate(rule)}>
                      {t("rules.row.duplicate")}
                    </MenuItem>
                    <MenuItem destructive onSelect={() => setDeleting(rule)}>
                      {t("rules.row.delete")}
                    </MenuItem>
                  </MenuContent>
                </Menu>
              </div>
            ))}
          </div>
        )}
      </div>

      {/* Builder dialog (create/edit). Keyed so a fresh draft mounts per target. */}
      {builderOpen && (
        <RuleBuilder
          token={token}
          rule={editing}
          open={builderOpen}
          onOpenChange={setBuilderOpen}
          onSaved={load}
        />
      )}

      {/* Delete confirm */}
      <Dialog open={!!deleting} onOpenChange={(o) => !o && setDeleting(null)}>
        <DialogContent>
          <DialogHeader>{t("rules.delete.title")}</DialogHeader>
          <DialogBody>
            <p className="text-body-sm text-text-secondary">
              {t("rules.delete.body", {
                name: deleting?.name || t("rules.row.unnamed"),
              })}
            </p>
          </DialogBody>
          <DialogFooter>
            <Button variant="ghost" onClick={() => setDeleting(null)}>
              {t("rules.delete.cancel")}
            </Button>
            <Button variant="danger" onClick={() => void confirmDelete()}>
              {t("rules.delete.confirm")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* F6 shadow confirm — only opened when >=1 fallback is actually active
       * (AC-122). variant="primary": recoverable, not "danger" — this may be
       * exactly what the operator wants. */}
      <ConfirmDialog
        open={shadowConfirmOpen}
        onClose={() => setShadowConfirmOpen(false)}
        title={t("rules.default.shadow.title")}
        body={
          <>
            <p>{t("rules.default.shadow.body")}</p>
            <ul className="mt-2 list-disc space-y-1 pl-5">
              {shadowBullets.map((bullet) => (
                <li key={bullet}>{bullet}</li>
              ))}
            </ul>
            <p className="mt-2 text-caption text-text-tertiary">
              {t("rules.default.shadow.recover")}
            </p>
          </>
        }
        confirmLabel={t("rules.default.shadow.confirm")}
        confirmVariant="primary"
        errorFallback={t("rules.default.error")}
        onConfirm={handleAddDefaultConfirmed}
      />
    </AppShell>
  );
}

/** F6 "Add default rule" (design.md §3.8, AC-124). `variant="secondary"` —
 * `variant="primary"` is the single accent button per surface and "New rule"
 * already owns it. Disabled-with-a-reason needs BOTH a mouse and a keyboard
 * path: a `disabled` `<button>` fires no pointer events and is out of tab
 * order, so the Tooltip wraps a focusable `<span>` around it instead of the
 * button itself, with `title` as a no-JS fallback. */
function AddDefaultRuleButton({
  disabled,
  reason,
  loading,
  onClick,
  className,
}: {
  disabled: boolean;
  reason: string | null;
  loading: boolean;
  onClick: () => void;
  className?: string;
}) {
  const button = (
    <Button
      variant="secondary"
      size="sm"
      className={className}
      disabled={disabled}
      loading={loading}
      onClick={onClick}
      aria-label={t("rules.default.aria")}
    >
      {loading ? t("rules.default.adding") : t("rules.default.add")}
    </Button>
  );
  if (!reason) return button;
  return (
    <Tooltip content={reason}>
      <span tabIndex={0} title={reason} className="inline-flex rounded-sm">
        {button}
      </span>
    </Tooltip>
  );
}
