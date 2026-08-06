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
import { Pause, Play } from "lucide-react";
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
import { AppShell } from "@/components/hookbox/app-shell";
import { SplitPane } from "@/components/hookbox/split-pane";
import { FeedRow } from "@/components/hookbox/feed-row";
import { ConnectionPill } from "@/components/hookbox/connection-pill";
import { CodeBlock } from "@/components/hookbox/code-block";
import { InlineAlert } from "@/components/hookbox/inline-alert";
import { Button } from "@/components/ui/button";
import { SkeletonLines } from "@/components/ui/skeleton";
import { Inspector } from "./dashboard/inspector";

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
  const [paused, setPaused] = useState(false);
  const [selectedId, setSelectedId] = useState<number | null>(null);
  // Track which selected ids came from the live stream (their detail may 404 →
  // pending) vs. a reconciled historical row (a 404 there is a real error).
  const liveIds = useRef<Set<number>>(new Set());
  const [loaded, setLoaded] = useState(false);

  const { rows, connState, attempt, newCount, flushBuffered } = useFeed({
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
}) {
  const list = rows;
  return (
    <div className="flex h-full min-h-0 flex-col border-r border-border">
      {/* Feed header — title · count · pause/resume */}
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
