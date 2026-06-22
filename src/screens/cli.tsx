/**
 * CLI / tunnel doc page (/cli) — PRD §3, AC-J8/J9, copy.md §5.11. A public-ish
 * documentation page describing the reverse-tunnel CLI (terminal-only — HookBox
 * never runs it; this page only documents it). When a session + endpoint exist
 * it fills the run command with the real token; the owner secret is masked
 * behind a Reveal/Hide toggle with a sharing warning (cli.secret.warning), since
 * the secret is the owner capability.
 *
 * The dashboard reflects tunnel_active live (handled on the dashboard via
 * endpoint_updated); this page is the static how-to, plus the behavior notes
 * (resolution order, no-tunnel 504, single-binding takeover, reconnect/backoff).
 */
import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { Eye, EyeOff } from "lucide-react";
import { api, session } from "@/api";
import { t } from "@/lib/copy";
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

export function Cli() {
  const [token, setToken] = useState<string | null>(null);
  const [revealed, setRevealed] = useState(false);
  const secret = session.getSecret();

  // Resolve the primary endpoint token for the command, if signed in.
  useEffect(() => {
    if (!secret) return;
    let cancelled = false;
    api
      .listEndpoints()
      .then((eps) => {
        if (!cancelled && eps.length > 0) setToken(eps[0].token);
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [secret]);

  const tokenSlot = token ?? "<endpoint>";
  // The command (display + copy) shows the real secret only once revealed, so a
  // copy carries the live capability; masked otherwise (cli.secret.warning).
  const secretSlot = secret ? (revealed ? secret : "•".repeat(24)) : "<secret>";
  const command = t("cli.command.template", {
    port: t("cli.command.portDefault"),
    token: tokenSlot,
    secret: secretSlot,
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
            {secret && (
              <Button
                variant="ghost"
                size="sm"
                onClick={() => setRevealed((r) => !r)}
                aria-pressed={revealed}
              >
                {revealed ? (
                  <EyeOff className="h-3.5 w-3.5" aria-hidden="true" />
                ) : (
                  <Eye className="h-3.5 w-3.5" aria-hidden="true" />
                )}
                {revealed ? t("cli.secret.hide") : t("cli.secret.reveal")}
              </Button>
            )}
          </div>
          {/* Reveal to copy with the live secret (masked by default). */}
          <CodeBlock value={command} ariaLabel={t("cli.command.copy.aria")} />
          {secret && (
            <InlineAlert variant="warning" role="status">
              {t("cli.secret.warning")}
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
