/**
 * Endpoint settings (/d/:token/settings) — PRD §5.2 #5/#6/#14/#16/#17/#18,
 * AC-49..52 (FE reflection), AC-J9, copy.md §5.9. A config form plus the
 * destructive operations:
 *
 *   #5  PATCH endpoint config (name, target_url, default_mode, auto_crud,
 *       latency_ms, rate_limit_per_min, chaos_pct, chaos_mode, cors_enabled)
 *   #14 clear request history     (set.retention.clearHistory)
 *   #16 clear endpoint state      (set.retention.clearState)
 *   #17 peek a collection         (set.crud.peek)
 *   #18 clear a collection        (set.crud.peek.clear)
 *   #6  DELETE endpoint behind a TYPED-TOKEN confirm (set.confirm.delete.*) →
 *       tombstones (410 on the mock surface) and routes back to a primary/landing
 *
 * States: loading · error+Retry · ready. Save is explicit (a single primary
 * Save); destructive ops each sit behind their own confirm dialog. All strings
 * from copy.md set.* via t().
 */
import { useCallback, useEffect, useRef, useState } from "react";
import { Link, useNavigate, useParams } from "react-router-dom";
import { ArrowLeft, RotateCw, Trash2 } from "lucide-react";
import { z } from "zod";
import {
  api,
  ApiError,
  mockRuleSchema,
  session,
  type EndpointConfigPatch,
  type EndpointDetail,
  type EndpointSummary,
} from "@/api";
import { t } from "@/lib/copy";
import { absolutize } from "@/lib/url";
import { cn } from "@/lib/cn";
import { downloadBlob } from "@/lib/download";
import {
  buildBundle,
  computeConfigDiff,
  countRulesUsingRequestHeaderTag,
  parseBundle,
  toBundleEndpoint,
  MAX_BUNDLE_BYTES,
  type ConfigBundle,
  type ConfigDiffRow,
} from "@/lib/config-bundle";
import { AppShell } from "@/components/hookbox/app-shell";
import { CodeBlock } from "@/components/hookbox/code-block";
import { ConfirmDialog } from "@/components/hookbox/confirm-dialog";
import { InlineAlert } from "@/components/hookbox/inline-alert";
import { JsonTree } from "@/components/hookbox/json-tree";
import { Button, buttonVariants } from "@/components/ui/button";
import { Field, Input, Label } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import { Slider } from "@/components/ui/slider";
import { Segmented } from "@/components/ui/segmented";
import { Progress } from "@/components/ui/progress";
import {
  Dialog,
  DialogBody,
  DialogContent,
  DialogFooter,
  DialogHeader,
} from "@/components/ui/dialog";
import { useToast } from "@/components/ui/toast";

type LoadState =
  | { kind: "loading" }
  | { kind: "error" }
  | { kind: "ready"; endpoint: EndpointDetail };

const COLLECTION_RE = /^[A-Za-z0-9_-]{1,64}$/;

export function Settings() {
  const { token = "" } = useParams();
  const navigate = useNavigate();
  const { toast } = useToast();
  const hasSession = !!session.getSecret();

  const [load, setLoad] = useState<LoadState>({ kind: "loading" });
  const [endpoints, setEndpoints] = useState<EndpointSummary[]>([]);
  // AC-89: bumped only after a successful import, so SettingsForm's
  // reset-effect can distinguish "server truth changed out from under the
  // form" from an ordinary Save (which already matches what's on screen).
  // NOT a remount key — a remount would also destroy ConfigurationSection's
  // own in-flight progress/report state, which lives inside SettingsForm's
  // tree so it can read the form's `dirty` flag.
  const [importGen, setImportGen] = useState(0);

  const fetchAll = useCallback(async () => {
    setLoad({ kind: "loading" });
    try {
      const endpoint = await api.getEndpoint(token);
      setLoad({ kind: "ready", endpoint });
    } catch (err) {
      if (err instanceof ApiError && err.status === 401) return;
      setLoad({ kind: "error" });
    }
  }, [token]);

  useEffect(() => {
    if (!hasSession) return;
    void fetchAll();
    api
      .listEndpoints()
      .then(setEndpoints)
      .catch(() => {});
  }, [hasSession, fetchAll]);

  if (!hasSession) {
    navigate("/", { replace: true, state: { reason: t("common.error.401") } });
    return null;
  }

  const endpoint = load.kind === "ready" ? load.endpoint : null;

  return (
    <AppShell token={token} endpoint={endpoint} endpoints={endpoints}>
      <div className="mx-auto h-full max-w-2xl overflow-auto p-6">
        <div className="mb-4 flex items-center gap-3">
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
          <h1 className="text-h2 text-text-primary">{t("set.title")}</h1>
        </div>

        {load.kind === "loading" && (
          <div aria-busy="true" className="space-y-3">
            <span className="sr-only">{t("set.loading.aria")}</span>
            <div className="skeleton h-9 w-full" />
            <div className="skeleton h-9 w-2/3" />
            <div className="skeleton h-9 w-1/2" />
          </div>
        )}

        {load.kind === "error" && (
          <InlineAlert
            variant="danger"
            role="alert"
            title={t("set.error.title")}
            action={
              <Button
                variant="secondary"
                size="sm"
                onClick={() => void fetchAll()}
              >
                <RotateCw className="h-3.5 w-3.5" aria-hidden="true" />
                {t("set.error.retry")}
              </Button>
            }
          />
        )}

        {load.kind === "ready" && (
          <SettingsForm
            token={token}
            endpoint={load.endpoint}
            importGen={importGen}
            onSaved={(e) => setLoad({ kind: "ready", endpoint: e })}
            onImported={(e) => {
              // AC-89: refresh server truth AND signal SettingsForm to reset
              // its field state from the new `endpoint` prop it is about to
              // receive — a re-fetch alone leaves the on-screen fields stale.
              setLoad({ kind: "ready", endpoint: e });
              setImportGen((g) => g + 1);
            }}
          />
        )}
      </div>
    </AppShell>
  );
}

function SettingsForm({
  token,
  endpoint,
  importGen,
  onSaved,
  onImported,
}: {
  token: string;
  endpoint: EndpointDetail;
  /** Bumped only after a successful F3 import — see the reset effect below. */
  importGen: number;
  onSaved: (e: EndpointDetail) => void;
  onImported: (e: EndpointDetail) => void;
}) {
  const navigate = useNavigate();
  const { toast } = useToast();

  // Form working state (PATCH #5 fields).
  const [name, setName] = useState(endpoint.name ?? "");
  const [targetUrl, setTargetUrl] = useState(endpoint.target_url ?? "");
  const [defaultMode, setDefaultMode] = useState(endpoint.default_mode);
  const [autoCrud, setAutoCrud] = useState(endpoint.auto_crud);
  const [corsEnabled, setCorsEnabled] = useState(endpoint.cors_enabled);
  const [latency, setLatency] = useState(endpoint.latency_ms);
  const [rate, setRate] = useState(endpoint.rate_limit_per_min);
  const [chaosPct, setChaosPct] = useState(endpoint.chaos_pct);
  const [chaosMode, setChaosMode] = useState(endpoint.chaos_mode);
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [proxyError, setProxyError] = useState<string | null>(null);

  // AC-89: `useState(endpoint…)` above only seeds the fields ONCE, at mount —
  // a bare re-fetch after import leaves the on-screen fields stale, and the
  // next Save would then revert the very import that just ran. `importGen`
  // changing (never on an ordinary Save, which already matches what's on
  // screen) means the import applied server-side; reset every field from the
  // fresh `endpoint` prop that arrives alongside it.
  const prevImportGen = useRef(importGen);
  useEffect(() => {
    if (importGen === prevImportGen.current) return;
    prevImportGen.current = importGen;
    setName(endpoint.name ?? "");
    setTargetUrl(endpoint.target_url ?? "");
    setDefaultMode(endpoint.default_mode);
    setAutoCrud(endpoint.auto_crud);
    setCorsEnabled(endpoint.cors_enabled);
    setLatency(endpoint.latency_ms);
    setRate(endpoint.rate_limit_per_min);
    setChaosPct(endpoint.chaos_pct);
    setChaosMode(endpoint.chaos_mode);
  }, [importGen, endpoint]);

  // Destructive-op dialogs.
  const [confirm, setConfirm] = useState<null | "history" | "state" | "delete">(
    null,
  );

  // F3's export-dirty note (AC-87) and import-confirm dirty warning
  // (AC-S21) both need "does the on-screen form differ from the server?" —
  // the same nine fields Save itself would PATCH.
  const dirty =
    (name.trim() || null) !== endpoint.name ||
    (targetUrl.trim() || null) !== endpoint.target_url ||
    defaultMode !== endpoint.default_mode ||
    autoCrud !== endpoint.auto_crud ||
    corsEnabled !== endpoint.cors_enabled ||
    latency !== endpoint.latency_ms ||
    rate !== endpoint.rate_limit_per_min ||
    chaosPct !== endpoint.chaos_pct ||
    chaosMode !== endpoint.chaos_mode;

  function validateProxy(): boolean {
    if (!targetUrl.trim()) {
      setProxyError(null);
      return true;
    }
    try {
      const u = new URL(targetUrl.trim());
      if (!["http:", "https:"].includes(u.protocol) || !u.host) {
        setProxyError(t("set.proxy.url.invalid"));
        return false;
      }
    } catch {
      setProxyError(t("set.proxy.url.invalid"));
      return false;
    }
    setProxyError(null);
    return true;
  }

  async function save() {
    if (!validateProxy()) return;
    setSaving(true);
    setSaveError(null);
    const patch: EndpointConfigPatch = {
      name: name.trim() || null,
      target_url: targetUrl.trim() || null,
      default_mode: defaultMode,
      auto_crud: autoCrud,
      cors_enabled: corsEnabled,
      latency_ms: latency,
      rate_limit_per_min: rate,
      chaos_pct: chaosPct,
      chaos_mode: chaosMode,
    };
    try {
      const updated = await api.patchEndpoint(token, patch);
      onSaved(updated);
      toast(t("set.toast.saved"));
    } catch {
      setSaveError(t("set.error.save"));
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="space-y-8">
      {saveError && (
        <InlineAlert variant="danger" role="alert">
          {saveError}
        </InlineAlert>
      )}

      {/* Identity */}
      <Section title={t("set.identity.title")}>
        <Field
          label={t("set.identity.name.label")}
          helper={t("set.identity.name.helper")}
          render={(p) => (
            <Input
              id={p.id}
              aria-describedby={p.describedBy}
              placeholder={t("set.identity.name.placeholder")}
              value={name}
              onChange={(e) => setName(e.target.value)}
            />
          )}
        />
        <div className="space-y-1.5">
          <Label>{t("set.identity.mockUrl.label")}</Label>
          <CodeBlock
            value={absolutize(endpoint.mock_url)}
            ariaLabel={t("set.identity.mockUrl.label")}
          />
        </div>
        <div className="space-y-1.5">
          <Label>{t("set.identity.pathUrl.label")}</Label>
          <CodeBlock
            value={absolutize(endpoint.path_url)}
            ariaLabel={t("set.identity.pathUrl.label")}
          />
        </div>
        <div className="space-y-1.5">
          <Label>{t("set.identity.token.label")}</Label>
          <CodeBlock
            value={endpoint.token}
            ariaLabel={t("set.identity.token.label")}
          />
        </div>
      </Section>

      {/* Proxy target (MITM) */}
      <Section title={t("set.proxy.title")}>
        <Field
          label={t("set.proxy.url.label")}
          helper={t("set.proxy.url.helper")}
          error={proxyError}
          render={(p) => (
            <Input
              id={p.id}
              aria-describedby={p.describedBy}
              invalid={p.invalid}
              mono
              placeholder={t("set.proxy.url.placeholder")}
              value={targetUrl}
              onChange={(e) => setTargetUrl(e.target.value)}
              onBlur={validateProxy}
            />
          )}
        />
        <p className="text-caption text-text-tertiary">
          {t("set.proxy.url.note")}
        </p>
      </Section>

      {/* Auto-CRUD + collection peek/clear */}
      <Section title={t("set.crud.title")}>
        <ToggleRow
          label={t("set.crud.toggle.label")}
          checked={autoCrud}
          onChange={setAutoCrud}
        />
        <CollectionPeek token={token} />
      </Section>

      {/* Default response */}
      <Section title={t("set.default.title")}>
        <p className="text-body-sm text-text-tertiary">
          {t("set.default.helper")}
        </p>
        <Segmented
          aria-label={t("set.default.title")}
          options={[
            { value: "mock_404", label: t("set.default.mock404.label") },
            { value: "echo", label: t("set.default.echo.label") },
          ]}
          value={defaultMode}
          onChange={(v) => setDefaultMode(v)}
        />
        <p className="text-caption text-text-tertiary">
          {defaultMode === "echo"
            ? t("set.default.echo.helper")
            : t("set.default.mock404.helper")}
        </p>
      </Section>

      {/* Simulated conditions */}
      <Section title={t("set.cond.title")}>
        <p className="text-body-sm text-text-tertiary">
          {t("set.cond.helper")}
        </p>
        <NumberWithSlider
          label={t("set.cond.latency.label")}
          unit={t("set.cond.latency.unit")}
          helper={t("set.cond.latency.helper")}
          min={0}
          max={10000}
          value={latency}
          onChange={setLatency}
        />
        <NumberWithSlider
          label={t("set.cond.rateLimit.label")}
          unit={t("set.cond.rateLimit.unit")}
          helper={t("set.cond.rateLimit.helper")}
          min={0}
          max={6000}
          value={rate}
          onChange={setRate}
        />
        <NumberWithSlider
          label={t("set.cond.chaos.label")}
          unit={t("set.cond.chaos.unit")}
          helper={t("set.cond.chaos.helper")}
          min={0}
          max={100}
          value={chaosPct}
          onChange={setChaosPct}
        />
        <Field
          label={t("set.cond.chaosMode.label")}
          render={(p) => (
            <select
              id={p.id}
              value={chaosMode}
              onChange={(e) =>
                setChaosMode(e.target.value as "error" | "dropout")
              }
              className="h-9 w-full rounded-sm border border-border-strong bg-surface px-2 text-body-sm text-text-primary"
            >
              <option value="error">{t("set.cond.chaosMode.error")}</option>
              <option value="dropout">{t("set.cond.chaosMode.dropout")}</option>
            </select>
          )}
        />
      </Section>

      {/* Auto-CORS */}
      <Section title={t("set.cors.title")}>
        <ToggleRow
          label={t("set.cors.toggle.label")}
          checked={corsEnabled}
          onChange={setCorsEnabled}
        />
        <p className="text-caption text-text-tertiary">
          {t("set.cors.helper")}
        </p>
      </Section>

      {/* Save (the single primary action for the config form) */}
      <div className="flex justify-end">
        <Button variant="primary" onClick={save} loading={saving}>
          {saving ? t("set.saving") : t("set.save")}
        </Button>
      </div>

      {/* Configuration export / import (F3) */}
      <ConfigurationSection
        token={token}
        endpoint={endpoint}
        dirty={dirty}
        onImported={onImported}
      />

      {/* Retention & state */}
      <Section title={t("set.retention.title")}>
        <p className="text-body-sm text-text-tertiary">
          {t("set.retention.note")}
        </p>
        <div className="flex flex-wrap gap-2">
          <Button
            variant="secondary"
            size="sm"
            onClick={() => setConfirm("history")}
          >
            {t("set.retention.clearHistory")}
          </Button>
          <Button
            variant="secondary"
            size="sm"
            onClick={() => setConfirm("state")}
          >
            {t("set.retention.clearState")}
          </Button>
        </div>
        <p className="text-caption text-text-tertiary">
          {t("set.retention.stateNote")}
        </p>
      </Section>

      {/* Danger zone */}
      <div className="rounded-md border border-danger-fg/40 bg-danger-bg p-4">
        <h2 className="text-h4 text-danger-fg">{t("set.danger.title")}</h2>
        <div className="mt-3 flex items-center justify-between gap-4">
          <div>
            <p className="text-body-sm font-medium text-text-primary">
              {t("set.danger.delete.label")}
            </p>
            <p className="text-caption text-text-tertiary">
              {t("set.danger.delete.helper")}
            </p>
          </div>
          <Button
            variant="danger"
            size="sm"
            onClick={() => setConfirm("delete")}
          >
            <Trash2 className="h-3.5 w-3.5" aria-hidden="true" />
            {t("set.danger.delete.label")}
          </Button>
        </div>
      </div>

      {/* Clear-history confirm (#14) */}
      <ConfirmDialog
        open={confirm === "history"}
        onClose={() => setConfirm(null)}
        title={t("set.confirm.clearHistory.title")}
        body={t("set.confirm.clearHistory.body")}
        confirmLabel={t("set.confirm.clearHistory.confirm")}
        onConfirm={async () => {
          await api.clearRequests(token);
          toast(t("set.toast.historyCleared"));
        }}
      />

      {/* Clear-state confirm (#16) */}
      <ConfirmDialog
        open={confirm === "state"}
        onClose={() => setConfirm(null)}
        title={t("set.confirm.clearState.title")}
        body={t("set.confirm.clearState.body")}
        confirmLabel={t("set.confirm.clearState.confirm")}
        onConfirm={async () => {
          await api.clearState(token);
          toast(t("set.toast.stateCleared"));
        }}
      />

      {/* Typed-token delete confirm (#6) */}
      <DeleteDialog
        open={confirm === "delete"}
        token={token}
        onClose={() => setConfirm(null)}
        onDeleted={() => {
          toast(t("set.toast.deleted"));
          navigate("/", { replace: true });
        }}
      />
    </div>
  );
}

// ── F3 Configuration export / import ──

type ImportPhase =
  | { kind: "idle" }
  | {
      kind: "confirming";
      bundle: ConfigBundle;
      diff: ConfigDiffRow[];
      existingRuleCount: number;
    }
  | { kind: "applying"; label: string; value: number; max: number }
  | {
      kind: "report";
      variant: "success" | "danger";
      message: string;
      addedRules: number;
    };

const CONFIG_IMPORT_INPUT_ID = "cfg-import";

function ConfigurationSection({
  token,
  endpoint,
  dirty,
  onImported,
}: {
  token: string;
  endpoint: EndpointDetail;
  dirty: boolean;
  onImported: (e: EndpointDetail) => void;
}) {
  const { toast } = useToast();
  const [exporting, setExporting] = useState(false);
  const [exportError, setExportError] = useState<string | null>(null);
  const [importError, setImportError] = useState<string | null>(null);
  const [phase, setPhase] = useState<ImportPhase>({ kind: "idle" });
  const busy = exporting || phase.kind === "applying";

  // AC-87: always builds from freshly fetched server state, never the
  // in-memory form (the `dirty` caption below is the only mention of that).
  async function handleExport() {
    setExporting(true);
    setExportError(null);
    let fresh: EndpointDetail;
    try {
      fresh = await api.getEndpoint(token);
    } catch (err) {
      setExporting(false);
      setExportError(
        err instanceof ApiError && (err.status === 404 || err.status === 410)
          ? t("common.error.endpointGone")
          : t("set.config.export.error"),
      );
      return;
    }
    let rules;
    try {
      // Re-parse to the precise output type — the client's generic infers
      // the looser input type for schemas carrying zod defaults (same
      // workaround as rules-manager.tsx's `load`).
      rules = z.array(mockRuleSchema).parse(await api.listRules(token));
    } catch {
      setExporting(false);
      setExportError(t("set.config.export.error.rules"));
      return;
    }
    try {
      const bundle = buildBundle(fresh, rules);
      downloadBlob(
        `hookbox-config-${token}.json`,
        "application/json",
        JSON.stringify(bundle, null, 2),
      );
      toast(t("set.config.toast.exported"));
    } catch {
      setExportError(t("set.config.export.error"));
    } finally {
      setExporting(false);
    }
  }

  // AC-16: the WHOLE file is parsed and validated before any network write.
  function handleFileChange(e: React.ChangeEvent<HTMLInputElement>) {
    const file = e.target.files?.[0];
    // AC-90: reset so re-selecting the SAME file after a rejection still fires.
    e.target.value = "";
    if (!file) return; // a cancelled file dialog is a silent no-op
    setImportError(null);
    setPhase({ kind: "idle" });
    if (file.size === 0) {
      setImportError(t("set.config.import.invalid.empty"));
      return;
    }
    if (file.size > MAX_BUNDLE_BYTES) {
      setImportError(t("set.config.import.tooLarge"));
      return;
    }
    file
      .text()
      .then(async (text) => {
        const result = parseBundle(text);
        if (!result.success) {
          setImportError(result.message);
          return;
        }
        let existingRuleCount = 0;
        try {
          existingRuleCount = (await api.listRules(token)).length;
        } catch {
          // Best-effort: the confirm still shows a count rather than
          // failing the whole flow over a read that isn't the one that matters.
        }
        const diff = computeConfigDiff(
          toBundleEndpoint(endpoint),
          result.bundle.endpoint,
        );
        setPhase({
          kind: "confirming",
          bundle: result.bundle,
          diff,
          existingRuleCount,
        });
      })
      .catch(() => setImportError(t("set.config.import.invalid.json")));
  }

  // Frozen orchestration: one PATCH, then one POST per rule in array order,
  // stop-at-first-failure, no rollback, add-never-replace (AC-17/18/19).
  async function runImport(bundle: ConfigBundle) {
    const total = bundle.rules.length;
    setPhase({
      kind: "applying",
      label: t("set.config.import.progressConfig"),
      value: 0,
      max: total,
    });
    let updated: EndpointDetail;
    try {
      updated = await api.patchEndpoint(token, bundle.endpoint);
    } catch (err) {
      const detail =
        err instanceof ApiError ? err.message : t("common.error.generic");
      setPhase({
        kind: "report",
        variant: "danger",
        message: t("set.config.import.failedConfig", { detail }),
        addedRules: 0,
      });
      return;
    }
    for (let i = 0; i < total; i++) {
      setPhase({
        kind: "applying",
        label: t("set.config.import.progressRules", { i: i + 1, n: total }),
        value: i,
        max: total,
      });
      try {
        await api.createRule(token, bundle.rules[i]);
      } catch (err) {
        const detail =
          err instanceof ApiError ? err.message : t("common.error.generic");
        const ruleName = bundle.rules[i].name || t("rules.row.unnamed");
        onImported(updated); // AC-19: the config step DID apply
        setPhase({
          kind: "report",
          variant: "danger",
          message: t("set.config.import.failedRule", {
            done: i,
            total,
            index: i + 1,
            name: ruleName,
            detail,
          }),
          addedRules: i,
        });
        return;
      }
    }
    onImported(updated);
    toast(t("set.config.toast.imported"));
    setPhase({
      kind: "report",
      variant: "success",
      message:
        total > 0
          ? t("set.config.import.done", { n: total })
          : t("set.config.import.done.noRules"),
      addedRules: total,
    });
  }

  return (
    <Section title={t("set.config.title")}>
      <p className="text-body-sm text-text-tertiary">
        {t("set.config.helper")}
      </p>

      {exportError && (
        <InlineAlert variant="danger" role="alert">
          {exportError}
        </InlineAlert>
      )}
      {importError && (
        <InlineAlert variant="danger" role="alert">
          <div className="space-y-2">
            <p>{importError}</p>
            <label
              htmlFor={CONFIG_IMPORT_INPUT_ID}
              className={cn(
                buttonVariants({ variant: "secondary", size: "sm" }),
                "cursor-pointer",
              )}
            >
              {t("set.config.import.chooseAnother")}
            </label>
          </div>
        </InlineAlert>
      )}

      <div className="flex flex-wrap gap-2">
        <Button
          variant="secondary"
          size="sm"
          onClick={() => void handleExport()}
          loading={exporting}
          disabled={busy}
        >
          {exporting ? t("set.config.export.busy") : t("set.config.export")}
        </Button>
        {/* The sr-only input must precede its label in the DOM for the
         * peer-focus-visible ring to work (design.md §3.4/AC-92). */}
        <input
          id={CONFIG_IMPORT_INPUT_ID}
          type="file"
          accept="application/json,.json"
          className="peer sr-only"
          disabled={busy}
          onChange={handleFileChange}
          aria-label={t("set.config.import.aria")}
        />
        <label
          htmlFor={CONFIG_IMPORT_INPUT_ID}
          className={cn(
            buttonVariants({ variant: "secondary", size: "sm" }),
            "cursor-pointer",
            "peer-focus-visible:outline peer-focus-visible:outline-2 peer-focus-visible:outline-offset-2 peer-focus-visible:outline-focus",
            busy && "peer-disabled:cursor-not-allowed peer-disabled:bg-surface-subtle peer-disabled:text-text-tertiary",
          )}
        >
          {t("set.config.import")}
        </label>
      </div>
      {dirty && (
        <p className="text-caption text-text-tertiary">
          {t("set.config.export.dirty")}
        </p>
      )}
      <p className="text-caption text-text-tertiary">
        {t("set.config.import.helper")}
      </p>
      <p className="text-caption text-text-tertiary">
        {t("set.config.import.fileHint")}
      </p>

      {phase.kind === "applying" && (
        <div className="space-y-1.5">
          <p className="text-body-sm text-text-secondary">{phase.label}</p>
          <Progress value={phase.value} max={phase.max} label={phase.label} />
          <p className="text-caption text-text-tertiary">
            {t("set.config.import.dontClose")}
          </p>
        </div>
      )}

      {phase.kind === "report" && (
        <InlineAlert
          variant={phase.variant === "danger" ? "danger" : "info"}
          role={phase.variant === "danger" ? "alert" : "status"}
          action={
            <div className="flex flex-wrap gap-2">
              {phase.addedRules > 0 && (
                <Button variant="secondary" size="sm" asChild>
                  <Link to={`/d/${token}/rules`}>
                    {t("set.config.import.viewRules")}
                  </Link>
                </Button>
              )}
              <Button
                variant="ghost"
                size="sm"
                onClick={() => setPhase({ kind: "idle" })}
              >
                {t("common.dismiss")}
              </Button>
            </div>
          }
        >
          {phase.message}
        </InlineAlert>
      )}

      <ConfirmDialog
        open={phase.kind === "confirming"}
        onClose={() => setPhase({ kind: "idle" })}
        title={t("set.config.confirm.title")}
        body={
          phase.kind === "confirming" ? (
            <ImportConfirmBody
              bundle={phase.bundle}
              diff={phase.diff}
              existingRuleCount={phase.existingRuleCount}
              dirty={dirty}
            />
          ) : null
        }
        confirmLabel={t("set.config.confirm.confirm")}
        confirmVariant="primary"
        onConfirm={async () => {
          if (phase.kind !== "confirming") return;
          void runImport(phase.bundle);
        }}
      />
    </Section>
  );
}

// AC-S21's pre-apply diff: provenance -> changing fields only -> the
// target_url consequence when present -> how many rules get added -> the
// dirty-form warning when applicable.
function ImportConfirmBody({
  bundle,
  diff,
  existingRuleCount,
  dirty,
}: {
  bundle: ConfigBundle;
  diff: ConfigDiffRow[];
  existingRuleCount: number;
  dirty: boolean;
}) {
  const when = new Date(bundle.exported_at).toLocaleString();
  const headerTagCount = countRulesUsingRequestHeaderTag(bundle.rules);
  const targetUrlChanged = diff.some((row) => row.field === "target_url");

  return (
    <div className="space-y-3">
      <p className="text-body-sm text-text-secondary">
        {bundle.endpoint.name
          ? t("set.config.confirm.exported", { when, name: bundle.endpoint.name })
          : t("set.config.confirm.exported.unnamed", { when })}
      </p>

      {diff.length === 0 ? (
        <p className="text-body-sm text-text-tertiary">
          {t("set.config.diff.none")}
        </p>
      ) : (
        <div className="space-y-1.5">
          <p className="text-body-sm font-medium text-text-primary">
            {t("set.config.diff.title", { n: diff.length })}
          </p>
          <div className="divide-y divide-border overflow-hidden rounded-sm border border-border">
            {diff.map((row) => (
              <div
                key={row.field}
                className={cn(
                  "grid grid-cols-[8rem_minmax(0,1fr)] gap-2 px-2.5 py-1.5",
                  row.field === "target_url" && "bg-warning-bg",
                )}
              >
                <span className="font-mono text-mono-sm text-text-secondary">
                  {row.field}
                </span>
                <span
                  className="min-w-0 break-all font-mono text-mono-sm"
                  aria-label={t("set.config.diff.change.aria", {
                    field: row.field,
                    from: row.from ?? t("set.config.diff.empty"),
                    to: row.to ?? t("set.config.diff.empty"),
                  })}
                >
                  <span
                    className={cn(
                      "line-through",
                      row.from === null && "not-italic",
                      "text-text-tertiary",
                    )}
                  >
                    {row.from ?? t("set.config.diff.empty")}
                  </span>
                  <span className="px-1 text-text-tertiary" aria-hidden="true">
                    →
                  </span>
                  <span className="text-text-primary">
                    {row.to ?? t("set.config.diff.empty")}
                  </span>
                </span>
              </div>
            ))}
          </div>
          {targetUrlChanged && (
            <p className="text-caption text-warning-fg">
              {t("set.config.diff.targetUrl.warning")}
            </p>
          )}
          <p className="text-caption text-text-tertiary">
            {t("set.config.diff.unchangedNote")}
          </p>
        </div>
      )}

      <p className="text-body-sm text-text-secondary">
        {bundle.rules.length > 0
          ? t("set.config.confirm.rules", {
              n: bundle.rules.length,
              existing: existingRuleCount,
            })
          : t("set.config.confirm.rules.none")}
      </p>

      {headerTagCount > 0 && (
        <p className="text-caption text-warning-fg">
          {t("set.config.confirm.headerTagWarning", { n: headerTagCount })}
        </p>
      )}

      {dirty && (
        <p className="text-body-sm text-warning-fg">
          {t("set.config.confirm.dirty")}
        </p>
      )}
    </div>
  );
}

// ── Collection peek / clear (#17 / #18) ──
function CollectionPeek({ token }: { token: string }) {
  const { toast } = useToast();
  const [nameInput, setNameInput] = useState("");
  const [items, setItems] = useState<Record<string, unknown>[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [active, setActive] = useState<string | null>(null);

  async function peek() {
    const name = nameInput.trim();
    if (!COLLECTION_RE.test(name)) {
      setError(t("set.crud.peek.invalid"));
      return;
    }
    setError(null);
    setBusy(true);
    try {
      const res = await api.peekCollection(token, name);
      setItems(res.items);
      setActive(name);
    } catch {
      setError(t("set.crud.peek.invalid"));
    } finally {
      setBusy(false);
    }
  }

  async function clear() {
    if (!active) return;
    try {
      await api.clearCollection(token, active);
      toast(t("set.toast.collectionCleared"));
      setItems([]);
    } catch {
      setError(t("set.crud.peek.invalid"));
    }
  }

  return (
    <div className="space-y-2">
      <Label>{t("set.crud.peek.label")}</Label>
      <div className="flex items-center gap-2">
        <Input
          mono
          placeholder={t("set.crud.peek.placeholder")}
          value={nameInput}
          onChange={(e) => setNameInput(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && void peek()}
          invalid={!!error}
        />
        <Button
          variant="secondary"
          size="sm"
          onClick={() => void peek()}
          loading={busy}
        >
          {t("set.crud.peek.view")}
        </Button>
      </div>
      {error && (
        <p className="text-body-sm text-danger-fg" role="alert">
          {error}
        </p>
      )}
      {items !== null && (
        <div className="space-y-2">
          <div className="flex items-center justify-between">
            <span className="text-caption text-text-tertiary">
              {t("set.crud.peek.count", { n: items.length })}
            </span>
            {active && (
              <Button variant="ghost" size="sm" onClick={() => void clear()}>
                {t("set.crud.peek.clear")}
              </Button>
            )}
          </div>
          {items.length === 0 ? (
            <p className="text-body-sm text-text-tertiary">
              {t("set.crud.peek.empty")}
            </p>
          ) : (
            <JsonTree raw={JSON.stringify(items)} />
          )}
        </div>
      )}
    </div>
  );
}

// ── Typed-token delete (#6) ──
function DeleteDialog({
  open,
  token,
  onClose,
  onDeleted,
}: {
  open: boolean;
  token: string;
  onClose: () => void;
  onDeleted: () => void;
}) {
  const [typed, setTyped] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const matches = typed.trim() === token;

  async function confirmDelete() {
    if (!matches) return;
    setBusy(true);
    setError(null);
    try {
      await api.deleteEndpoint(token);
      onDeleted();
    } catch {
      setError(t("set.error.delete"));
    } finally {
      setBusy(false);
    }
  }

  return (
    <Dialog
      open={open}
      onOpenChange={(o) => {
        if (!o) {
          setTyped("");
          setError(null);
          onClose();
        }
      }}
    >
      <DialogContent>
        <DialogHeader>{t("set.confirm.delete.title", { token })}</DialogHeader>
        <DialogBody className="space-y-3">
          <p className="text-body-sm text-text-secondary">
            {t("set.confirm.delete.body")}
          </p>
          {error && (
            <InlineAlert variant="danger" role="alert">
              {error}
            </InlineAlert>
          )}
          <Field
            label={t("set.confirm.delete.prompt")}
            render={(p) => (
              <Input
                id={p.id}
                mono
                placeholder={t("set.confirm.delete.placeholder", { token })}
                value={typed}
                onChange={(e) => setTyped(e.target.value)}
              />
            )}
          />
        </DialogBody>
        <DialogFooter>
          <Button variant="ghost" onClick={onClose}>
            {t("set.confirm.cancel")}
          </Button>
          <Button
            variant="danger"
            disabled={!matches}
            loading={busy}
            onClick={() => void confirmDelete()}
          >
            {t("set.confirm.delete.confirm")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function Section({
  title,
  children,
}: {
  title: string;
  children: React.ReactNode;
}) {
  return (
    <section className="space-y-3 border-b border-border pb-6 last:border-b-0">
      <h2 className="text-h4 text-text-primary">{title}</h2>
      {children}
    </section>
  );
}

function ToggleRow({
  label,
  checked,
  onChange,
}: {
  label: string;
  checked: boolean;
  onChange: (v: boolean) => void;
}) {
  return (
    <label className="flex items-center justify-between gap-4">
      <span className="text-body-sm text-text-primary">{label}</span>
      <Switch checked={checked} onCheckedChange={onChange} aria-label={label} />
    </label>
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
