/**
 * Dashboard (/d/:token) — the core operator screen (PRD §3, AC-44/45,
 * AC-D13/D18/D19, AC-J2/J3/J4/J10). A split-screen: the live feed on the left,
 * the deep inspector on the right.
 *
 * SHELL states (AC-J2, copy.md dash.state.*):
 *   not-signed-in → bounce to / (no secret)
 *   loading       → GET /api/endpoints/{token} in flight (dash.state.loading.aria)
 *   not-found     → 404 unknown_endpoint card (distinct from gone, OQ-1)
 *   gone          → 410 endpoint_gone card (distinct copy)
 *   offline       → browser offline banner over the last-known data (AC-J13)
 *   loaded        → the split-screen
 *
 * FEED states (AC-41/J2, copy.md feed.*): loading skeleton · empty (mock-URL
 * CodeBlock + a static, never-executed curl sample) · streaming (newest-first,
 * capped 100, feed.count) · paused (+ "N new" pill, buffered, flush on resume).
 * The first new_request transitions empty→streaming with no reload (useFeed).
 *
 * The connection pill (feed.conn.*) lives in the sub-header via headerExtra.
 * endpoint_updated reflects tunnel_active live (AC-J10 / dash.tunnel.active).
 */
import { useCallback, useEffect, useRef, useState } from "react";
import { Navigate, useNavigate, useParams } from "react-router-dom";
import { MoreHorizontal, Pause, Play } from "lucide-react";
import {
  api,
  ApiError,
  session,
  type EndpointDetail,
  type EndpointSummary,
  type RequestSummary,
} from "@/api";
import { useFeed, connLabel, connTooltip } from "@/feed";
import { t } from "@/lib/copy";
import { cn } from "@/lib/cn";
import { absolutize } from "@/lib/url";
import { remapEndpointGone } from "@/lib/api-errors";
import { downloadBlob } from "@/lib/download";
import {
  buildRequestCsv,
  exportFilename,
  fetchDetails,
  isSentinelCell,
  type DetailCell,
} from "@/lib/request-export";
import { AppShell } from "@/components/hookbox/app-shell";
import { SplitPane } from "@/components/hookbox/split-pane";
import { FeedRow } from "@/components/hookbox/feed-row";
import { ConnectionPill } from "@/components/hookbox/connection-pill";
import { CodeBlock } from "@/components/hookbox/code-block";
import { ConfirmDialog } from "@/components/hookbox/confirm-dialog";
import { InlineAlert } from "@/components/hookbox/inline-alert";
import { Button } from "@/components/ui/button";
import {
  Menu,
  MenuContent,
  MenuItem,
  MenuSeparator,
  MenuTrigger,
} from "@/components/ui/menu";
import { Progress } from "@/components/ui/progress";
import { SkeletonLines } from "@/components/ui/skeleton";
import { useToast } from "@/components/ui/toast";
import { Inspector } from "./dashboard/inspector";

type ExportPhase =
  | { kind: "idle" }
  | { kind: "fetching"; done: number; total: number }
  | { kind: "serialising"; total: number };

type Shell =
  | { kind: "loading" }
  | { kind: "not-found" }
  | { kind: "gone" }
  | { kind: "loaded"; endpoint: EndpointDetail };

export function Dashboard() {
  const { token = "" } = useParams();
  const navigate = useNavigate();
  const hasSession = !!session.getSecret();

  const [shell, setShell] = useState<Shell>({ kind: "loading" });
  const [endpoints, setEndpoints] = useState<EndpointSummary[]>([]);
  const [tunnelActive, setTunnelActive] = useState<boolean | undefined>(
    undefined,
  );
  const [offline, setOffline] = useState(!navigator.onLine);

  // Load the endpoint detail (the shell gate) + sibling list (the switcher).
  const loadShell = useCallback(async () => {
    setShell({ kind: "loading" });
    try {
      const detail = await api.getEndpoint(token);
      setShell({ kind: "loaded", endpoint: detail });
      setTunnelActive(detail.tunnel_active);
    } catch (err) {
      if (err instanceof ApiError) {
        if (err.status === 404) {
          setShell({ kind: "not-found" });
          return;
        }
        if (err.status === 410) {
          setShell({ kind: "gone" });
          return;
        }
        // 401 already bounced via the client; leave the loading state (the
        // navigate to / will unmount this screen).
      }
      // Network/other: surface the offline banner but keep the loading frame.
      if (!navigator.onLine) setOffline(true);
    }
  }, [token]);

  useEffect(() => {
    if (!hasSession) return;
    void loadShell();
    api
      .listEndpoints()
      .then(setEndpoints)
      .catch(() => {
        /* the switcher just shows the current token if listing fails */
      });
  }, [hasSession, loadShell]);

  useEffect(() => {
    const on = () => {
      setOffline(false);
      // Reconcile the shell when connectivity returns.
      if (shell.kind !== "loaded") void loadShell();
    };
    const off = () => setOffline(true);
    window.addEventListener("online", on);
    window.addEventListener("offline", off);
    return () => {
      window.removeEventListener("online", on);
      window.removeEventListener("offline", off);
    };
  }, [shell.kind, loadShell]);

  // endpoint_updated → re-fetch the affected fields (tunnel_active reflection).
  const onEndpointUpdated = useCallback(
    (fields: string[]) => {
      if (fields.includes("tunnel_active") || fields.length === 0) {
        api
          .getEndpoint(token)
          .then((d) => {
            setTunnelActive(d.tunnel_active);
            setShell((prev) =>
              prev.kind === "loaded" ? { kind: "loaded", endpoint: d } : prev,
            );
          })
          .catch(() => {
            /* transient; the live pill already reflects connection health */
          });
      }
    },
    [token],
  );

  if (!hasSession) {
    return (
      <Navigate to="/" replace state={{ reason: t("common.error.401") }} />
    );
  }

  // Shell error cards (not-found / gone) — distinct copy (OQ-1).
  if (shell.kind === "not-found" || shell.kind === "gone") {
    const isGone = shell.kind === "gone";
    return (
      <div className="flex min-h-screen flex-col items-center justify-center gap-3 bg-canvas p-6 text-center">
        <h1 className="text-h2 text-text-primary">
          {isGone ? t("dash.state.gone.title") : t("dash.state.notFound.title")}
        </h1>
        <p className="max-w-md text-body-sm text-text-tertiary">
          {isGone ? t("dash.state.gone.body") : t("dash.state.notFound.body")}
        </p>
        <Button variant="secondary" onClick={() => navigate("/")}>
          {t("dash.state.backToStart")}
        </Button>
      </div>
    );
  }

  if (shell.kind === "loading") {
    return (
      <div
        className="flex min-h-screen items-center justify-center bg-canvas"
        aria-busy="true"
      >
        <span className="sr-only">{t("dash.state.loading.aria")}</span>
        <SkeletonLines lines={3} className="w-64" />
      </div>
    );
  }

  return (
    <DashboardLoaded
      token={token}
      endpoint={shell.endpoint}
      endpoints={endpoints}
      tunnelActive={tunnelActive}
      offline={offline}
      onEndpointUpdated={onEndpointUpdated}
    />
  );
}

function DashboardLoaded({
  token,
  endpoint,
  endpoints,
  tunnelActive,
  offline,
  onEndpointUpdated,
}: {
  token: string;
  endpoint: EndpointDetail;
  endpoints: EndpointSummary[];
  tunnelActive?: boolean;
  offline: boolean;
  onEndpointUpdated: (fields: string[]) => void;
}) {
  const { toast } = useToast();
  const [paused, setPaused] = useState(false);
  const [selectedId, setSelectedId] = useState<number | null>(null);
  // Track which selected ids came from the live stream (their detail may 404 →
  // pending) vs. a reconciled historical row (a 404 there is a real error).
  const liveIds = useRef<Set<number>>(new Set());
  const [loaded, setLoaded] = useState(false);
  const [confirmClearOpen, setConfirmClearOpen] = useState(false);
  // F5 export state (AC-48/117/120). `exporting` (AC-82's shared contract)
  // derives from this rather than a separate flag, so the two can never
  // drift out of sync.
  const [exportPhase, setExportPhase] = useState<ExportPhase>({
    kind: "idle",
  });
  const [exportDetailNote, setExportDetailNote] = useState(false);
  const exportController = useRef<AbortController | null>(null);
  const exporting = exportPhase.kind !== "idle";

  // AC-53: unmounting (e.g. a 401 bounce) aborts any in-flight export so no
  // late state update or download fires against a gone screen.
  useEffect(() => {
    return () => {
      exportController.current?.abort();
    };
  }, []);

  const { rows, connState, attempt, newCount, flushBuffered, clearRows } =
    useFeed({
      token,
      paused,
      onEndpointUpdated,
    });

  useEffect(() => {
    // Mark the loading→content transition: the first reconcile/new_request fills
    // rows. We treat "rows arrived OR connection settled" as loaded.
    if (rows.length > 0 || connState === "live" || connState === "sse")
      setLoaded(true);
  }, [rows.length, connState]);

  // Newly-streamed rows are flagged live so the inspector reads a 404 as pending.
  const prevTopId = useRef<number | null>(null);
  useEffect(() => {
    const top = rows[0]?.id ?? null;
    if (top != null && top !== prevTopId.current) {
      liveIds.current.add(top);
      prevTopId.current = top;
    }
  }, [rows]);

  function togglePause() {
    if (paused) flushBuffered();
    setPaused((p) => !p);
  }

  // F1 "Clear all" (AC-76): enabled iff there's anything to clear, accounting
  // for the paused buffer; disabled while an export is in flight or offline —
  // never two reasons stacked (offline takes precedence, then busy, then empty).
  const hasClearable = rows.length > 0 || newCount > 0;
  const clearAllDisabled = !hasClearable || exporting || offline;
  const clearAllHint = offline
    ? t("feed.actions.offlineHint")
    : exporting
      ? t("feed.actions.busyHint")
      : !hasClearable
        ? t("feed.actions.emptyHint")
        : null;

  async function handleClearAll() {
    try {
      await api.clearRequests(token);
    } catch (err) {
      toast(t("feed.clearAll.error"), "danger");
      remapEndpointGone(err);
    }
    clearRows();
    // AC-78: the Inspector must not sit on a row that no longer exists.
    setSelectedId((id) => {
      if (id != null) liveIds.current.delete(id);
      return null;
    });
    // AC-79: refresh request_count rather than leaving it stale — reuses the
    // endpoint_updated "refetch everything" path (empty fields list).
    onEndpointUpdated([]);
    toast(t("set.toast.historyCleared"));
  }

  // F5 "Export CSV" (AC-46..56, AC-115..121). Snapshot rows at activation
  // (AC-115/116) — buffered ("N new") arrivals are excluded because they are
  // not visible, and later arrivals never join the run.
  async function handleExportCsv() {
    const snapshot = rows.slice();
    const total = snapshot.length;
    const controller = new AbortController();
    exportController.current = controller;
    setExportDetailNote(false);
    setExportPhase({ kind: "fetching", done: 0, total });

    let details: ReadonlyArray<DetailCell>;
    try {
      details = await fetchDetails(snapshot, controller.signal, (done) => {
        if (!controller.signal.aborted) {
          setExportPhase({ kind: "fetching", done, total });
        }
      });
    } catch {
      if (exportController.current === controller) {
        setExportPhase({ kind: "idle" });
        toast(t("feed.export.error"), "danger");
      }
      return;
    }

    if (controller.signal.aborted) {
      setExportPhase({ kind: "idle" });
      toast(t("feed.export.cancelled"));
      return;
    }

    // AC-117: "Preparing file…", Cancel disabled (not removed) — this phase
    // is synchronous and cannot itself be interrupted.
    setExportPhase({ kind: "serialising", total });
    try {
      const csv = buildRequestCsv(snapshot, details);
      downloadBlob(
        exportFilename(token, new Date()),
        "text/csv;charset=utf-8",
        csv,
      );
    } catch {
      setExportPhase({ kind: "idle" });
      toast(t("feed.export.error.file"), "danger");
      return;
    }

    setExportPhase({ kind: "idle" });
    const missing = details.filter(isSentinelCell).length;
    if (missing > 0) {
      setExportDetailNote(true);
      toast(t("feed.export.done.partial", { n: total, m: missing }));
    } else {
      toast(t("feed.export.done", { n: total }));
    }
  }

  function cancelExport() {
    exportController.current?.abort();
  }

  const pill = (
    <ConnectionPill
      state={connState}
      label={connLabel(connState, attempt)}
      title={connTooltip(connState)}
    />
  );

  const feed = (
    <FeedPane
      rows={rows}
      mockUrl={endpoint.mock_url}
      loaded={loaded}
      paused={paused}
      newCount={newCount}
      selectedId={selectedId}
      onSelect={setSelectedId}
      onTogglePause={togglePause}
      onFlush={flushBuffered}
      clearAllDisabled={clearAllDisabled}
      clearAllHint={clearAllHint}
      onClearAllSelect={() => setConfirmClearOpen(true)}
      exportPhase={exportPhase}
      onExportSelect={() => void handleExportCsv()}
      onExportCancel={cancelExport}
      exportDetailNote={exportDetailNote}
      onDismissExportDetailNote={() => setExportDetailNote(false)}
    />
  );

  return (
    <AppShell
      token={token}
      endpoint={endpoint}
      endpoints={endpoints}
      tunnelActive={tunnelActive}
      headerExtra={pill}
    >
      {offline && (
        <div className="border-b border-border px-4 py-2">
          <InlineAlert
            variant="warning"
            role="status"
            title={t("dash.state.offline.title")}
          >
            {t("dash.state.offline.body")}
          </InlineAlert>
        </div>
      )}
      <SplitPane
        className="h-full"
        left={feed}
        right={
          <Inspector
            id={selectedId}
            isLive={selectedId != null && liveIds.current.has(selectedId)}
          />
        }
      />
      <ConfirmDialog
        open={confirmClearOpen}
        onClose={() => setConfirmClearOpen(false)}
        title={t("feed.clearAll.confirm.title")}
        body={
          <>
            <p>
              {t("feed.clearAll.confirm.body", {
                endpoint: endpoint.name || endpoint.token,
              })}
            </p>
            <p className="mt-2 text-caption text-text-tertiary">
              {t("feed.clearAll.confirm.note")}
            </p>
          </>
        }
        confirmLabel={t("feed.clearAll.confirm.confirm")}
        errorFallback={t("feed.clearAll.error")}
        onConfirm={handleClearAll}
      />
    </AppShell>
  );
}

function FeedPane({
  rows,
  mockUrl,
  loaded,
  paused,
  newCount,
  selectedId,
  onSelect,
  onTogglePause,
  onFlush,
  clearAllDisabled,
  clearAllHint,
  onClearAllSelect,
  exportPhase,
  onExportSelect,
  onExportCancel,
  exportDetailNote,
  onDismissExportDetailNote,
}: {
  rows: RequestSummary[];
  mockUrl: string;
  loaded: boolean;
  paused: boolean;
  newCount: number;
  selectedId: number | null;
  onSelect: (id: number) => void;
  onTogglePause: () => void;
  onFlush: () => void;
  /** AC-76/AC-82: enable predicate shared by both feed-actions menu items. */
  clearAllDisabled: boolean;
  /** Exactly one reason (empty/busy/offline), or null when both items are
   * enabled with nothing to explain. */
  clearAllHint: string | null;
  onClearAllSelect: () => void;
  exportPhase: ExportPhase;
  onExportSelect: () => void;
  onExportCancel: () => void;
  /** AC-121: at least one row in the last completed export had no detail. */
  exportDetailNote: boolean;
  onDismissExportDetailNote: () => void;
}) {
  const list = rows;
  const exportLabel =
    exportPhase.kind === "fetching"
      ? t("feed.export.progress", {
          done: exportPhase.done,
          total: exportPhase.total,
        })
      : exportPhase.kind === "serialising"
        ? t("feed.export.preparing")
        : "";
  const exportValue =
    exportPhase.kind === "fetching" ? exportPhase.done : 0;
  const exportTotal = exportPhase.kind !== "idle" ? exportPhase.total : 0;
  return (
    <div className="flex h-full min-h-0 flex-col border-r border-border">
      {/* Feed header — title · count · actions menu · pause/resume */}
      <div className="flex items-center justify-between gap-2 border-b border-border px-3 py-2">
        <div className="flex items-center gap-2">
          <h2 className="text-h4 text-text-primary">
            {t("feed.header.title")}
          </h2>
          {loaded && list.length > 0 && (
            <span className="text-caption text-text-tertiary">
              {t("feed.count", { n: list.length })}
            </span>
          )}
        </div>
        <div className="flex items-center gap-2">
          {paused && newCount > 0 && (
            <Button
              variant="secondary"
              size="sm"
              onClick={onFlush}
              aria-label={t("feed.newCount.aria", { n: newCount })}
            >
              {t("feed.newCount", { n: newCount })}
            </Button>
          )}
          {/* Overflow menu — the trigger stays enabled even when every item is
           * disabled, so keyboard users can still open it and hear why
           * (AC-1). Export CSV above the separator, Clear all (destructive)
           * last. */}
          <Menu>
            <MenuTrigger asChild>
              <Button
                variant="ghost"
                size="icon-sm"
                aria-label={t("feed.actions.menu.aria")}
              >
                <MoreHorizontal className="h-4 w-4" aria-hidden="true" />
              </Button>
            </MenuTrigger>
            <MenuContent align="end">
              <MenuItem
                disabled={clearAllDisabled}
                aria-label={t("feed.export.aria")}
                onSelect={onExportSelect}
              >
                {t("feed.export")}
              </MenuItem>
              <MenuSeparator />
              <MenuItem
                destructive
                disabled={clearAllDisabled}
                aria-label={t("feed.clearAll.aria")}
                onSelect={onClearAllSelect}
              >
                {t("feed.clearAll")}
              </MenuItem>
              {clearAllHint && (
                <div className="px-2 py-1.5 text-caption text-text-tertiary">
                  {clearAllHint}
                </div>
              )}
            </MenuContent>
          </Menu>
          <Button
            variant="ghost"
            size="sm"
            onClick={onTogglePause}
            aria-label={paused ? t("feed.resume.aria") : t("feed.pause.aria")}
            aria-pressed={paused}
          >
            {paused ? (
              <Play className="h-3.5 w-3.5" aria-hidden="true" />
            ) : (
              <Pause className="h-3.5 w-3.5" aria-hidden="true" />
            )}
            {paused ? t("feed.resume") : t("feed.pause")}
          </Button>
        </div>
      </div>

      {/* Export progress strip (design.md §3.3) — non-modal, between the
       * header and the list; the feed keeps streaming underneath. */}
      {exportPhase.kind !== "idle" && (
        <div className="animate-fade-in space-y-1.5 border-b border-border bg-accent-subtle-bg px-3 py-2">
          <div className="flex items-center justify-between gap-2">
            <span className="tnum text-body-sm text-text-secondary">
              {exportLabel}
            </span>
            <Button
              variant="ghost"
              size="sm"
              onClick={onExportCancel}
              disabled={exportPhase.kind === "serialising"}
              aria-label={t("feed.export.cancel.aria")}
            >
              {t("feed.export.cancel")}
            </Button>
          </div>
          <Progress
            value={exportPhase.kind === "serialising" ? exportTotal : exportValue}
            max={exportTotal}
            label={exportLabel}
          />
        </div>
      )}

      {/* AC-121: persistent (dismissible) note when the last completed
       * export had at least one row without detail — a toast is too
       * short-lived to explain the file's sentinels. */}
      {exportDetailNote && (
        <div className="border-b border-border px-3 py-2">
          <InlineAlert
            variant="info"
            role="status"
            action={
              <Button
                variant="ghost"
                size="sm"
                onClick={onDismissExportDetailNote}
              >
                {t("common.dismiss")}
              </Button>
            }
          >
            {t("feed.export.detailNote")}
          </InlineAlert>
        </div>
      )}

      {/* Body — loading skeleton · empty · streaming list */}
      <div className="min-h-0 flex-1 overflow-auto">
        {!loaded ? (
          <div className="p-3" aria-busy="true">
            <span className="sr-only">{t("feed.loading.aria")}</span>
            <SkeletonLines lines={8} />
          </div>
        ) : list.length === 0 ? (
          <FeedEmpty mockUrl={mockUrl} />
        ) : (
          <div
            role="listbox"
            aria-label={t("feed.header.title")}
            className={cn("flex flex-col")}
          >
            {list.map((row, i) => (
              <FeedRow
                key={row.id}
                row={row}
                selected={row.id === selectedId}
                isNew={i === 0}
                onSelect={onSelect}
              />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

function FeedEmpty({ mockUrl }: { mockUrl: string }) {
  const displayUrl = absolutize(mockUrl);
  return (
    <div className="flex flex-col items-center gap-4 px-6 py-12 text-center">
      <div className="space-y-1">
        <h3 className="text-h4 text-text-primary">{t("feed.empty.title")}</h3>
        <p className="max-w-sm text-body-sm text-text-tertiary">
          {t("feed.empty.body")}
        </p>
      </div>
      <div className="w-full max-w-md space-y-2 text-left">
        <p className="text-caption font-medium uppercase tracking-wide text-text-tertiary">
          {t("dash.mockUrl.label")}
        </p>
        <CodeBlock value={displayUrl} ariaLabel={t("dash.mockUrl.label")} />
        <p className="pt-2 text-caption font-medium uppercase tracking-wide text-text-tertiary">
          {t("feed.empty.sampleLabel")}
        </p>
        {/* A static, illustrative curl sample — copy-only, never executed. */}
        <CodeBlock
          value={t("feed.empty.sample", { mock_url: displayUrl })}
          ariaLabel={t("feed.empty.sample", { mock_url: displayUrl })}
        />
        <p className="text-caption text-text-tertiary">
          {t("feed.empty.sampleHint")}
        </p>
      </div>
    </div>
  );
}
