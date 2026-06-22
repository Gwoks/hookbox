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
} from "@/api";
import { z } from "zod";
import { t } from "@/lib/copy";
import { AppShell } from "@/components/hookbox/app-shell";
import { MethodBadge } from "@/components/hookbox/method-badge";
import { InlineAlert } from "@/components/hookbox/inline-alert";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import { SkeletonLines } from "@/components/ui/skeleton";
import { Menu, MenuContent, MenuItem, MenuTrigger } from "@/components/ui/menu";
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
          <Button variant="primary" size="sm" onClick={openNew}>
            <Plus className="h-4 w-4" aria-hidden="true" />
            {t("rules.newRule")}
          </Button>
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
          </div>
        )}

        {state.kind === "ready" && state.rules.length > 0 && (
          <div className="overflow-hidden rounded-md border border-border bg-surface">
            {/* Column header — aligns with each row's grid (AC-D18). */}
            <div className="grid grid-cols-[4rem_1fr_auto_3rem_2rem] items-center gap-2 border-b border-border bg-subtle px-3 py-2 text-overline uppercase tracking-wide text-text-tertiary">
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
                    <MenuItem
                      className="text-danger-fg"
                      onSelect={() => setDeleting(rule)}
                    >
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
    </AppShell>
  );
}
