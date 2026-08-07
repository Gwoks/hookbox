/**
 * The public /s/:code viewer (operator-toolkit F4, PRD §3/§4.4, design.md
 * §3.7). Anyone holding the URL — no account, no session, no owner secret,
 * and this screen never creates one (AC-41/AC-42/AC-S13). This module's
 * import graph MUST NOT reach src/api/session.ts: it never imports
 * AppShell, SplitPane, ConnectionPill, FeedRow or FeedEmpty (all
 * owner-only/owner-voiced), and it renders zero accent-filled controls
 * (AC-109). Chrome is copied from /cli — BrandMark (not a link) + ThemeToggle
 * only — not AppShell's app bar.
 */
import { useEffect, useState } from "react";
import { Link, useParams } from "react-router-dom";
import { RotateCw } from "lucide-react";
import { t } from "@/lib/copy";
import { cn } from "@/lib/cn";
import { relTime } from "@/lib/time";
import { BrandMark } from "@/components/hookbox/brand-mark";
import { InlineAlert } from "@/components/hookbox/inline-alert";
import { Button } from "@/components/ui/button";
import { SkeletonLines } from "@/components/ui/skeleton";
import { ThemeToggle } from "@/theme/theme";
import { useSharedFeed } from "./share-view/use-shared-feed";
import { SharedRequestRow } from "./share-view/row";

const COLUMN_GRID =
  "grid-cols-[1rem_3.5rem_minmax(0,1fr)_auto_auto_auto_auto]";

export function ShareView() {
  const { code = "" } = useParams();
  const { status, data, retryInSeconds, refresh } = useSharedFeed(code);
  const [openRowId, setOpenRowId] = useState<number | null>(null);
  const [isHidden, setIsHidden] = useState(
    () => document.visibilityState === "hidden",
  );
  const [isOffline, setIsOffline] = useState(() => !navigator.onLine);
  const [countdown, setCountdown] = useState<number | null>(null);

  useEffect(() => {
    document.title = t("viewer.docTitle");
  }, []);

  useEffect(() => {
    const onVis = () => setIsHidden(document.visibilityState === "hidden");
    const onOnline = () => setIsOffline(false);
    const onOffline = () => setIsOffline(true);
    document.addEventListener("visibilitychange", onVis);
    window.addEventListener("online", onOnline);
    window.addEventListener("offline", onOffline);
    return () => {
      document.removeEventListener("visibilitychange", onVis);
      window.removeEventListener("online", onOnline);
      window.removeEventListener("offline", onOffline);
    };
  }, []);

  useEffect(() => {
    if (status !== "rateLimited" || retryInSeconds == null) {
      setCountdown(null);
      return;
    }
    setCountdown(retryInSeconds);
    const interval = setInterval(() => {
      setCountdown((c) => (c && c > 0 ? c - 1 : 0));
    }, 1000);
    return () => clearInterval(interval);
  }, [status, retryInSeconds]);

  // AC-106's terminal state — no banner, no card, header + footer retained.
  if (status === "unavailable") {
    return <UnavailablePage />;
  }

  return (
    <div className="flex min-h-screen flex-col bg-canvas">
      <a
        href="#main"
        className="sr-only focus:not-sr-only focus:absolute focus:left-2 focus:top-2"
      >
        {t("shell.skipLink")}
      </a>

      <header className="flex items-center justify-between px-6 py-4">
        <BrandMark />
        <ThemeToggle />
      </header>

      {/* AC-107: standing, non-dismissible banner spanning full width. */}
      <div className="border-y border-border bg-surface px-4 py-3">
        <InlineAlert
          variant="info"
          role="status"
          title={t("viewer.banner.title")}
          className="mx-auto max-w-viewer"
        >
          {t("viewer.banner.body")}
        </InlineAlert>
      </div>

      <main
        id="main"
        className="mx-auto w-full max-w-viewer flex-1 space-y-4 px-4 py-6"
      >
        <div className="space-y-1">
          {/* AC-107: the h1 is ALWAYS the static string — never the endpoint name. */}
          <h1 className="text-h1 text-text-primary">{t("viewer.title")}</h1>
          {data && (
            <p className="truncate text-body-sm text-text-secondary">
              <span title={data.endpoint.name ?? undefined}>
                {t("viewer.subject.name", {
                  name: data.endpoint.name || t("viewer.subject.unnamed"),
                })}
              </span>
              {" · "}
              {t("viewer.subject.total", { n: data.endpoint.request_count })}
            </p>
          )}
        </div>

        {isOffline && (
          <InlineAlert variant="info" role="status" title={t("viewer.offline.title")}>
            {t("viewer.offline.body")}
          </InlineAlert>
        )}

        {!isOffline && status === "rateLimited" && (
          <InlineAlert variant="warning" role="status" title={t("viewer.rateLimited.title")}>
            {t("viewer.rateLimited.body", { seconds: countdown ?? retryInSeconds ?? 0 })}
          </InlineAlert>
        )}

        {!isOffline && status === "error" && (
          <InlineAlert
            variant="danger"
            role="alert"
            title={t("viewer.error.title")}
            action={
              <Button variant="secondary" size="sm" onClick={refresh}>
                <RotateCw className="h-3.5 w-3.5" aria-hidden="true" />
                {t("common.retry")}
              </Button>
            }
          >
            {t("viewer.error.body")}
          </InlineAlert>
        )}

        <div className="overflow-hidden rounded-md border border-border bg-surface shadow-xs">
          <div className="flex flex-wrap items-center justify-between gap-2 border-b border-border px-3 py-2">
            <div className="flex items-center gap-2">
              <span className="text-body-sm text-text-secondary">
                {data ? t("viewer.count", { n: data.requests.length }) : ""}
              </span>
              <span className="inline-flex rounded-xs bg-neutral-chip-bg px-1.5 py-0.5 text-caption font-medium text-neutral-chip-fg">
                {t("viewer.readOnlyChip")}
              </span>
            </div>
            <div className="flex items-center gap-2">
              {data && (
                <span
                  className={cn(
                    "text-caption",
                    status === "rateLimited" ? "text-warning-fg" : "text-text-tertiary",
                  )}
                  title={new Date(data.updatedAt).toISOString()}
                >
                  {t("viewer.updated", { when: relTime(new Date(data.updatedAt).toISOString()) })}
                </span>
              )}
              <Button
                variant="secondary"
                size="sm"
                onClick={refresh}
                aria-label={t("viewer.refresh.aria")}
              >
                <RotateCw className="h-3.5 w-3.5" aria-hidden="true" />
                {t("viewer.refresh")}
              </Button>
            </div>
          </div>

          {data && data.requests.length > 0 && (
            <div
              className={cn(
                "hidden gap-2 border-b border-border bg-surface-subtle px-3 py-2 text-overline uppercase tracking-wide text-text-tertiary sm:grid",
                COLUMN_GRID,
              )}
            >
              <span aria-hidden="true" />
              <span>{t("viewer.col.method")}</span>
              <span>{t("viewer.col.path")}</span>
              <span>{t("viewer.col.status")}</span>
              <span>{t("viewer.col.served")}</span>
              <span>{t("viewer.col.duration")}</span>
              <span>{t("viewer.col.when")}</span>
            </div>
          )}

          {!data && status === "loading" && (
            <div className="p-3" aria-busy="true">
              <span className="sr-only">{t("viewer.loading.aria")}</span>
              <SkeletonLines lines={8} />
            </div>
          )}

          {data && data.requests.length === 0 && (
            <div className="px-6 py-12 text-center">
              <h2 className="text-h4 text-text-primary">{t("viewer.empty.title")}</h2>
              <p className="mx-auto mt-2 max-w-sm text-body-sm text-text-tertiary">
                {t("viewer.empty.body")}
              </p>
            </div>
          )}

          {data && data.requests.length > 0 && (
            <div>
              {data.requests.map((row) => (
                <SharedRequestRow
                  key={row.id}
                  code={code}
                  row={row}
                  isOpen={openRowId === row.id}
                  onToggle={() =>
                    setOpenRowId((id) => (id === row.id ? null : row.id))
                  }
                />
              ))}
            </div>
          )}
        </div>

        <p className="text-caption text-text-tertiary">
          {isHidden ? t("viewer.updating.paused") : t("viewer.updating")}
        </p>
      </main>

      <footer className="border-t border-border px-6 py-6">
        <p className="mx-auto max-w-viewer text-caption text-text-tertiary">
          {t("viewer.footer")}
        </p>
      </footer>
    </div>
  );
}

function UnavailablePage() {
  useEffect(() => {
    document.title = t("viewer.docTitle");
  }, []);
  return (
    <div className="flex min-h-screen flex-col bg-canvas">
      <header className="flex items-center justify-between px-6 py-4">
        <BrandMark />
        <ThemeToggle />
      </header>
      <main
        id="main"
        className="flex flex-1 flex-col items-center justify-center gap-3 p-6 text-center"
      >
        <h1 className="text-h2 text-text-primary">{t("viewer.unavailable.title")}</h1>
        <p className="max-w-md text-body-sm text-text-tertiary">
          {t("viewer.unavailable.body")}
        </p>
        <Button variant="link" asChild>
          <Link to="/">{t("viewer.unavailable.about")}</Link>
        </Button>
      </main>
      <footer className="border-t border-border px-6 py-6 text-center text-caption text-text-tertiary">
        {t("viewer.footer")}
      </footer>
    </div>
  );
}
