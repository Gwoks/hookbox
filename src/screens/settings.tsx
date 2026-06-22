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
import { useCallback, useEffect, useState } from "react";
import { Link, useNavigate, useParams } from "react-router-dom";
import { ArrowLeft, RotateCw, Trash2 } from "lucide-react";
import {
  api,
  ApiError,
  session,
  type EndpointConfigPatch,
  type EndpointDetail,
  type EndpointSummary,
} from "@/api";
import { t } from "@/lib/copy";
import { AppShell } from "@/components/hookbox/app-shell";
import { CodeBlock } from "@/components/hookbox/code-block";
import { InlineAlert } from "@/components/hookbox/inline-alert";
import { JsonTree } from "@/components/hookbox/json-tree";
import { Button } from "@/components/ui/button";
import { Field, Input, Label } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import { Slider } from "@/components/ui/slider";
import { Segmented } from "@/components/ui/segmented";
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
            onSaved={(e) => setLoad({ kind: "ready", endpoint: e })}
          />
        )}
      </div>
    </AppShell>
  );
}

function SettingsForm({
  token,
  endpoint,
  onSaved,
}: {
  token: string;
  endpoint: EndpointDetail;
  onSaved: (e: EndpointDetail) => void;
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

  // Destructive-op dialogs.
  const [confirm, setConfirm] = useState<null | "history" | "state" | "delete">(
    null,
  );

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
            value={endpoint.mock_url}
            ariaLabel={t("set.identity.mockUrl.label")}
          />
        </div>
        <div className="space-y-1.5">
          <Label>{t("set.identity.pathUrl.label")}</Label>
          <CodeBlock
            value={endpoint.path_url}
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
        body={t("set.confirm.clearHistory.body", { n: endpoint.request_count })}
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

// ── Generic confirm (history / state) ──
function ConfirmDialog({
  open,
  onClose,
  title,
  body,
  confirmLabel,
  onConfirm,
}: {
  open: boolean;
  onClose: () => void;
  title: string;
  body: string;
  confirmLabel: string;
  onConfirm: () => Promise<void>;
}) {
  const [busy, setBusy] = useState(false);
  return (
    <Dialog open={open} onOpenChange={(o) => !o && onClose()}>
      <DialogContent>
        <DialogHeader>{title}</DialogHeader>
        <DialogBody>
          <p className="text-body-sm text-text-secondary">{body}</p>
        </DialogBody>
        <DialogFooter>
          <Button variant="ghost" onClick={onClose}>
            {t("set.confirm.cancel")}
          </Button>
          <Button
            variant="danger"
            loading={busy}
            onClick={async () => {
              setBusy(true);
              try {
                await onConfirm();
                onClose();
              } finally {
                setBusy(false);
              }
            }}
          >
            {confirmLabel}
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
