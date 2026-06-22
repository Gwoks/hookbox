/**
 * AppShell — the dashboard chrome (design.md §3 / §7, copy.md §5.1/§5.2). A
 * top app bar (brand · endpoint switcher · account menu · theme toggle) plus a
 * sub-header carrying the endpoint subject: mock URL + local path (copy-only
 * MockUrlChips, AC-D19), the Auto-CRUD glance, the live tunnel-active badge
 * (dash.tunnel.active, reflected from endpoint_updated), and the Rules / New
 * rule / Settings actions. The body fills the rest of the viewport.
 *
 * Strings come from copy.md §5.1/§5.2 via t(); no copy lives here. The switcher
 * + account menu use the Radix-backed Menu primitive.
 */
import type { ReactNode } from "react";
import { Link, useNavigate } from "react-router-dom";
import {
  Database,
  Plus,
  RadioTower,
  Settings as SettingsIcon,
} from "lucide-react";
import { BrandMark } from "./brand-mark";
import { MockUrlChip } from "./code-block";
import { Button } from "@/components/ui/button";
import {
  Menu,
  MenuContent,
  MenuItem,
  MenuSeparator,
  MenuTrigger,
} from "@/components/ui/menu";
import { Tooltip } from "@/components/ui/tooltip";
import { ThemeToggle } from "@/theme/theme";
import {
  session,
  useSession,
  type EndpointDetail,
  type EndpointSummary,
} from "@/api";
import { t } from "@/lib/copy";
import { cn } from "@/lib/cn";

export interface AppShellProps {
  token: string;
  /** The current endpoint detail (null while the sub-header is still loading). */
  endpoint: EndpointDetail | null;
  /** Sibling endpoints for the switcher (empty until listed). */
  endpoints: EndpointSummary[];
  /** Live tunnel-active reflection (endpoint_updated); overrides endpoint.tunnel_active. */
  tunnelActive?: boolean;
  /** Right-aligned slot in the sub-header (e.g. the connection pill). */
  headerExtra?: ReactNode;
  children: ReactNode;
}

export function AppShell({
  token,
  endpoint,
  endpoints,
  tunnelActive,
  headerExtra,
  children,
}: AppShellProps) {
  const navigate = useNavigate();
  const snap = useSession();
  const tunnelOn = tunnelActive ?? endpoint?.tunnel_active ?? false;

  function signOut() {
    session.clear();
    navigate("/", { replace: true });
  }

  return (
    <div className="flex h-screen min-h-0 flex-col bg-canvas">
      <a
        href="#main"
        className="sr-only focus:not-sr-only focus:absolute focus:left-2 focus:top-2"
      >
        {t("shell.skipLink")}
      </a>

      {/* Top bar */}
      <header className="flex items-center justify-between gap-3 border-b border-border bg-surface px-4 py-2">
        <div className="flex min-w-0 items-center gap-3">
          <BrandMark />
          <EndpointSwitcher
            token={token}
            endpoint={endpoint}
            endpoints={endpoints}
          />
        </div>
        <div className="flex items-center gap-2">
          <ThemeToggle />
          <AccountMenu email={snap.email} onSignOut={signOut} />
        </div>
      </header>

      {/* Sub-header — the endpoint subject + actions */}
      <div className="flex flex-wrap items-center justify-between gap-x-4 gap-y-2 border-b border-border bg-surface px-4 py-2.5">
        <div className="flex min-w-0 flex-wrap items-center gap-x-4 gap-y-1.5">
          <UrlChip label={t("dash.mockUrl.label")} url={endpoint?.mock_url} />
          <UrlChip label={t("dash.pathUrl.label")} url={endpoint?.path_url} />
          {endpoint?.auto_crud && (
            <Tooltip content={t("dash.autoCrud.tooltip")}>
              <span className="inline-flex items-center gap-1 rounded-xs bg-subtle px-1.5 py-0.5 text-caption font-medium text-text-secondary">
                <Database className="h-3 w-3" aria-hidden="true" />
                {t("dash.autoCrud.label")}
              </span>
            </Tooltip>
          )}
          {tunnelOn && (
            <Tooltip content={t("dash.tunnel.active.tooltip")}>
              <span className="inline-flex items-center gap-1 rounded-pill bg-served-tunnel-bg px-2 py-0.5 text-caption font-medium text-served-tunnel-fg">
                <RadioTower className="h-3 w-3" aria-hidden="true" />
                {t("dash.tunnel.active")}
              </span>
            </Tooltip>
          )}
        </div>
        <div className="flex items-center gap-2">
          {headerExtra}
          <Button variant="ghost" size="sm" asChild>
            <Link to={`/d/${token}/rules`}>{t("dash.action.rules")}</Link>
          </Button>
          <Button
            variant="ghost"
            size="sm"
            asChild
            aria-label={t("dash.action.settings")}
          >
            <Link to={`/d/${token}/settings`}>
              <SettingsIcon className="h-4 w-4" aria-hidden="true" />
              <span className="sr-only sm:not-sr-only">
                {t("dash.action.settings")}
              </span>
            </Link>
          </Button>
          <Button variant="primary" size="sm" asChild>
            <Link to={`/d/${token}/rules?new=1`}>
              <Plus className="h-4 w-4" aria-hidden="true" />
              {t("dash.action.newRule")}
            </Link>
          </Button>
        </div>
      </div>

      <main id="main" className="min-h-0 flex-1 overflow-hidden">
        {children}
      </main>
    </div>
  );
}

function UrlChip({ label, url }: { label: string; url?: string }) {
  return (
    <span className="flex min-w-0 items-center gap-1.5">
      <span className="text-caption font-medium uppercase tracking-wide text-text-tertiary">
        {label}
      </span>
      {url ? (
        <MockUrlChip url={url} />
      ) : (
        <span
          className="h-5 w-40 animate-pulse rounded-xs bg-subtle"
          aria-hidden="true"
        />
      )}
    </span>
  );
}

function EndpointSwitcher({
  token,
  endpoint,
  endpoints,
}: {
  token: string;
  endpoint: EndpointDetail | null;
  endpoints: EndpointSummary[];
}) {
  const navigate = useNavigate();
  const current = endpoint?.name || endpoint?.token || token;

  return (
    <Menu>
      <MenuTrigger asChild>
        <Button
          variant="secondary"
          size="sm"
          className="max-w-[14rem]"
          aria-label={t("shell.nav.endpointSwitcher.label")}
        >
          <span className="truncate font-mono text-mono-sm">{current}</span>
        </Button>
      </MenuTrigger>
      <MenuContent align="start">
        {endpoints.map((ep) => (
          <MenuItem
            key={ep.token}
            className={cn(ep.token === token && "text-text-primary")}
            onSelect={() => {
              if (ep.token !== token) navigate(`/d/${ep.token}`);
            }}
          >
            <span className="truncate font-mono text-mono-sm">
              {ep.name || ep.token}
            </span>
          </MenuItem>
        ))}
        {endpoints.length > 0 && <MenuSeparator />}
        <MenuItem
          onSelect={() => navigate("/d/__new__")}
          aria-label={t("shell.nav.newEndpoint.aria")}
        >
          <Plus className="h-3.5 w-3.5" aria-hidden="true" />
          {t("shell.nav.newEndpoint")}
        </MenuItem>
      </MenuContent>
    </Menu>
  );
}

function AccountMenu({
  email,
  onSignOut,
}: {
  email: string | null;
  onSignOut: () => void;
}) {
  return (
    <Menu>
      <MenuTrigger asChild>
        <Button variant="ghost" size="sm" aria-label={t("shell.account.label")}>
          {t("shell.account.label")}
        </Button>
      </MenuTrigger>
      <MenuContent align="end">
        {email && (
          <>
            <div className="px-2 py-1.5 text-caption text-text-tertiary">
              {t("shell.account.signedInAs", { email })}
            </div>
            <MenuSeparator />
          </>
        )}
        <MenuItem onSelect={onSignOut}>{t("shell.account.signOut")}</MenuItem>
      </MenuContent>
    </Menu>
  );
}
