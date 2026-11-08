/**
 * CLI / tunnel doc page (/cli) — PRD §3, AC-J8/J9, copy.md §5.11. A public-ish
 * documentation page describing the reverse-tunnel CLI (terminal-only — HookBox
 * never runs it; this page only documents it). When the browser is signed in
 * it fills the run command with the primary endpoint's token; the recovery key
 * (the tunnel's `--key`) stays masked until Reveal fetches it from
 * POST /api/session/key into local state only, with a sharing warning
 * (cli.key.warning). Hide drops it again. Signed out, the command shows
 * `<endpoint>` / `<recovery-key>` placeholders — this page never redirects.
 *
 * The dashboard reflects tunnel_active live (handled on the dashboard via
 * endpoint_updated); this page is the static how-to, plus the behavior notes
 * (resolution order, no-tunnel 504, single-binding takeover, reconnect/backoff).
 */
import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { Eye, EyeOff } from "lucide-react";
import { api, useSessionGate } from "@/api";
import { errorMessage } from "@/lib/api-errors";
import { t } from "@/lib/copy";
import { useAsyncAction } from "@/lib/hooks/use-async-action";
import { BrandMark } from "@/components/hookbox/brand-mark";
import { CodeBlock } from "@/components/hookbox/code-block";
import { InlineAlert } from "@/components/hookbox/inline-alert";
import { Button } from "@/components/ui/button";
import { ThemeToggle } from "@/theme/theme";

const BEHAVIOR: import("@/lib/copy").CopyKey[] = [
  "cli.behavior.order",
  "cli.behavior.noTunnel",
  "cli.behavior.takeover",
  "cli.behavior.reconnect",
];

const MASKED_KEY = "•".repeat(24);

export function Cli() {
  // Bootstraps the session (and migrates a legacy secret) but never
  // redirects: signed out just keeps the placeholders.
  const isSignedIn = useSessionGate().status === "signed-in";
  const [primaryToken, setPrimaryToken] = useState<string | null>(null);
  const [revealedKey, setRevealedKey] = useState<string | null>(null);
  const reveal = useAsyncAction(() => api.revealKey());

  // Resolve the primary endpoint token for the command, if signed in.
  useEffect(() => {
    if (!isSignedIn) return;
    let cancelled = false;
    api
      .getSession()
      .then((summary) => {
        if (!cancelled) setPrimaryToken(summary.primary?.token ?? null);
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [isSignedIn]);

  async function toggleReveal() {
    if (revealedKey) {
      setRevealedKey(null);
      return;
    }
    const result = await reveal.run();
    if (result.ok) setRevealedKey(result.value.key);
  }

  // Nothing owner-specific survives a sign-out in another tab.
  const token = isSignedIn ? primaryToken : null;
  const key = isSignedIn ? revealedKey : null;
  const tokenSlot = token ?? "<endpoint>";
  // The command (display + copy) carries the real key only once revealed;
  // masked otherwise (cli.key.warning).
  const keySlot = isSignedIn ? (key ?? MASKED_KEY) : t("cli.key.placeholder");
  // The tunnel binary defaults --host to ws://localhost:8080, which only works
  // for a local dev server; derive the real host from the page origin so the
  // copied command reaches this deployment (nginx proxies /ws/ in prod, Vite
  // proxies it in dev).
  const hostSlot = window.location.origin.replace(/^http/, "ws");
  const command = t("cli.command.template", {
    port: t("cli.command.portDefault"),
    token: tokenSlot,
    key: keySlot,
    host: hostSlot,
  });

  return (
    <div className="flex min-h-screen flex-col bg-canvas">
      <header className="flex items-center justify-between px-6 py-4">
        <BrandMark />
        <div className="flex items-center gap-2">
          {token && (
            <Button variant="link" asChild>
              <Link to={`/d/${token}`}>{t("dash.action.settings")}</Link>
            </Button>
          )}
          <ThemeToggle />
        </div>
      </header>

      <main
        id="main"
        className="mx-auto w-full max-w-landing flex-1 space-y-6 px-4 pb-16 pt-6"
      >
        <div className="space-y-2">
          <h1 className="text-display text-text-primary">{t("cli.title")}</h1>
          <p className="text-body text-text-secondary">{t("cli.intro")}</p>
        </div>

        <section className="space-y-2">
          <div className="flex items-center justify-between">
            <h2 className="text-h4 text-text-primary">
              {t("cli.command.label")}
            </h2>
            {isSignedIn && (
              <Button
                variant="ghost"
                size="sm"
                onClick={() => void toggleReveal()}
                aria-pressed={key !== null}
                loading={reveal.pending}
              >
                {key ? (
                  <EyeOff className="h-3.5 w-3.5" aria-hidden="true" />
                ) : (
                  <Eye className="h-3.5 w-3.5" aria-hidden="true" />
                )}
                {key ? t("cli.key.hide") : t("cli.key.reveal")}
              </Button>
            )}
          </div>
          {/* Reveal to copy with the live key (masked by default). */}
          <CodeBlock value={command} ariaLabel={t("cli.command.copy.aria")} />
          {reveal.error !== null && (
            <InlineAlert variant="danger" role="alert">
              {errorMessage(reveal.error, t("cli.key.error"))}
            </InlineAlert>
          )}
          {isSignedIn && (
            <InlineAlert variant="warning" role="status">
              {t("cli.key.warning")}
            </InlineAlert>
          )}
        </section>

        <section className="space-y-2">
          <h2 className="text-h4 text-text-primary">
            {t("cli.behavior.title")}
          </h2>
          <ul className="space-y-2">
            {BEHAVIOR.map((key) => (
              <li
                key={key}
                className="flex gap-2 text-body-sm text-text-secondary"
              >
                <span aria-hidden="true" className="text-text-tertiary">
                  •
                </span>
                {t(key)}
              </li>
            ))}
          </ul>
        </section>
      </main>
    </div>
  );
}
