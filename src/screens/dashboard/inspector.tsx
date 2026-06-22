/**
 * Inspector — the deep request detail (PRD §3, AC-44/45, copy.md §5.5). Driven
 * by a selected feed-row id; fetches GET /api/requests/{id} (§5.2 #13) and shows
 * one of the documented states (AC-J3/J4):
 *
 *   empty        — no row selected (insp.empty.*)
 *   loading      — fetch in flight (insp.loading.aria + skeleton)
 *   pending      — a JUST-streamed new_request whose detail 404s because the
 *                  trace isn't persisted yet → insp.pending.* + Retry, NOT a
 *                  hard 404 (OQ-2 / journey.md, AC-45)
 *   unauthorized — 401 (rotated cap) → insp.unauthorized.*
 *   error        — any other failure → insp.error.* + Retry
 *   ready        — 5 tabs Headers · Query · Body · Response Served · State &
 *                  Tracing, each with its own empty sub-state
 *
 * Caller passes `isLive` (true if the selected id is still only known from the
 * live feed, i.e. its detail may not be written yet) so a 404 is interpreted as
 * pending rather than gone. All strings via t(); values render as plain text
 * nodes (XSS-inert) through KeyValueRows / JsonTree / CodeBlock.
 */
import { useCallback, useEffect, useState } from "react";
import { RotateCw } from "lucide-react";
import { api, ApiError, type RequestDetail } from "@/api";
import { t } from "@/lib/copy";
import { Button } from "@/components/ui/button";
import { InlineAlert } from "@/components/hookbox/inline-alert";
import { SkeletonLines } from "@/components/ui/skeleton";
import { KeyValueRows } from "@/components/hookbox/key-value-rows";
import { JsonTree } from "@/components/hookbox/json-tree";
import { CodeBlock } from "@/components/hookbox/code-block";
import { ServedByChip } from "@/components/hookbox/served-by-chip";
import { MethodBadge } from "@/components/hookbox/method-badge";
import { StatusCode } from "@/components/hookbox/status-code";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";

type InspState =
  | { kind: "empty" }
  | { kind: "loading" }
  | { kind: "pending" }
  | { kind: "unauthorized" }
  | { kind: "error" }
  | { kind: "ready"; detail: RequestDetail };

export function Inspector({
  id,
  isLive,
}: {
  id: number | null;
  isLive: boolean;
}) {
  const [state, setState] = useState<InspState>({ kind: "empty" });

  const load = useCallback(
    async (requestId: number) => {
      setState({ kind: "loading" });
      try {
        const detail = await api.getRequest(requestId);
        setState({ kind: "ready", detail });
      } catch (err) {
        if (err instanceof ApiError) {
          // 404 on a freshly-streamed row = trace not yet persisted → pending,
          // not a hard 404 (AC-45 / journey.md OQ-2).
          if (err.status === 404 && isLive) {
            setState({ kind: "pending" });
            return;
          }
          if (err.status === 401) {
            // The client already cleared + bounced; reflect the local state too.
            setState({ kind: "unauthorized" });
            return;
          }
        }
        setState({ kind: "error" });
      }
    },
    [isLive],
  );

  useEffect(() => {
    if (id == null) {
      setState({ kind: "empty" });
      return;
    }
    void load(id);
  }, [id, load]);

  if (state.kind === "empty") {
    return (
      <Centered>
        <h2 className="text-h4 text-text-primary">{t("insp.empty.title")}</h2>
        <p className="text-body-sm text-text-tertiary">
          {t("insp.empty.body")}
        </p>
      </Centered>
    );
  }

  if (state.kind === "loading") {
    return (
      <div className="p-4" aria-busy="true">
        <span className="sr-only">{t("insp.loading.aria")}</span>
        <SkeletonLines lines={6} />
      </div>
    );
  }

  if (state.kind === "pending") {
    return (
      <div className="p-4">
        <InlineAlert
          variant="info"
          role="status"
          title={t("insp.pending.title")}
          action={
            id != null && (
              <Button
                variant="secondary"
                size="sm"
                onClick={() => void load(id)}
              >
                <RotateCw className="h-3.5 w-3.5" aria-hidden="true" />
                {t("insp.pending.retry")}
              </Button>
            )
          }
        >
          {t("insp.pending.body")}
        </InlineAlert>
      </div>
    );
  }

  if (state.kind === "unauthorized") {
    return (
      <div className="p-4">
        <InlineAlert
          variant="danger"
          role="alert"
          title={t("insp.unauthorized.title")}
        >
          {t("insp.unauthorized.body")}
        </InlineAlert>
      </div>
    );
  }

  if (state.kind === "error") {
    return (
      <div className="p-4">
        <InlineAlert
          variant="danger"
          role="alert"
          title={t("insp.error.title")}
          action={
            id != null && (
              <Button
                variant="secondary"
                size="sm"
                onClick={() => void load(id)}
              >
                <RotateCw className="h-3.5 w-3.5" aria-hidden="true" />
                {t("insp.error.retry")}
              </Button>
            )
          }
        >
          {t("insp.error.body")}
        </InlineAlert>
      </div>
    );
  }

  return <Ready detail={state.detail} />;
}

function Ready({ detail }: { detail: RequestDetail }) {
  return (
    <div className="flex h-full min-h-0 flex-col">
      {/* Subject strip — one subject, one color (design.md §2.3). */}
      <div className="flex flex-wrap items-center gap-2 border-b border-border px-4 py-2.5">
        <MethodBadge method={detail.method} />
        <span
          className="truncate font-mono text-mono-sm text-text-primary"
          title={detail.path}
        >
          {detail.path}
        </span>
        <StatusCode code={detail.status_code} />
        <ServedByChip servedBy={detail.served_by} />
      </div>

      <Tabs defaultValue="headers" className="flex min-h-0 flex-1 flex-col">
        <TabsList>
          <TabsTrigger value="headers">{t("insp.tab.headers")}</TabsTrigger>
          <TabsTrigger value="query">{t("insp.tab.query")}</TabsTrigger>
          <TabsTrigger value="body">{t("insp.tab.body")}</TabsTrigger>
          <TabsTrigger value="response">{t("insp.tab.response")}</TabsTrigger>
          <TabsTrigger value="trace">{t("insp.tab.trace")}</TabsTrigger>
        </TabsList>

        <div className="min-h-0 flex-1 overflow-auto p-4">
          <TabsContent value="headers">
            <KeyValueRows
              data={detail.request_headers}
              emptyLabel={t("insp.headers.empty")}
            />
          </TabsContent>

          <TabsContent value="query">
            <KeyValueRows
              data={detail.query_params}
              emptyLabel={t("insp.query.empty")}
            />
          </TabsContent>

          <TabsContent value="body">
            {detail.request_body ? (
              <JsonTree raw={detail.request_body} />
            ) : (
              <p className="text-body-sm text-text-tertiary">
                {t("insp.body.empty")}
              </p>
            )}
          </TabsContent>

          <TabsContent value="response" className="space-y-4">
            <section className="space-y-1.5">
              <h3 className="text-overline uppercase tracking-wide text-text-tertiary">
                {t("insp.response.servedByLabel")}
              </h3>
              <ServedByChip servedBy={detail.served_by} />
            </section>
            <section className="space-y-1.5">
              <h3 className="text-overline uppercase tracking-wide text-text-tertiary">
                {t("insp.response.headers")}
              </h3>
              <KeyValueRows
                data={detail.response_headers}
                emptyLabel={t("insp.headers.empty")}
              />
            </section>
            <section className="space-y-1.5">
              <h3 className="text-overline uppercase tracking-wide text-text-tertiary">
                {t("insp.response.body")}
              </h3>
              {detail.response_body ? (
                <JsonTree raw={detail.response_body} />
              ) : (
                <p className="text-body-sm text-text-tertiary">
                  {t("insp.response.empty")}
                </p>
              )}
            </section>
          </TabsContent>

          <TabsContent value="trace" className="space-y-4">
            <section className="space-y-1.5">
              <h3 className="text-overline uppercase tracking-wide text-text-tertiary">
                {t("insp.trace.title")}
              </h3>
              {detail.trace.length > 0 ? (
                <ol className="space-y-1">
                  {detail.trace.map((step, i) => (
                    <li
                      key={i}
                      className="flex gap-2 rounded-sm bg-subtle px-2 py-1.5 font-mono text-mono-sm"
                    >
                      <span className="shrink-0 font-medium text-text-secondary">
                        {step.step}
                      </span>
                      <span className="break-all text-text-primary">
                        {step.detail}
                      </span>
                    </li>
                  ))}
                </ol>
              ) : (
                <p className="text-body-sm text-text-tertiary">
                  {t("insp.trace.empty")}
                </p>
              )}
            </section>
            <section className="space-y-1.5">
              <h3 className="text-overline uppercase tracking-wide text-text-tertiary">
                {t("insp.trace.stateTitle")}
              </h3>
              <KeyValueRows
                data={detail.state_snapshot}
                emptyLabel={t("insp.trace.stateEmpty")}
              />
            </section>
          </TabsContent>
        </div>
      </Tabs>
    </div>
  );
}

function Centered({ children }: { children: React.ReactNode }) {
  return (
    <div className="flex h-full flex-col items-center justify-center gap-1.5 p-6 text-center">
      {children}
    </div>
  );
}
