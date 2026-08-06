/**
 * SPA route tree (PRD §3 surfaces): landing / email gate (/), split-screen
 * dashboard (/d/:token), rules manager + 5-tab rule builder, endpoint settings,
 * public tunnel/CLI page (/cli). Screen components are delivered by issues
 * .29–.32; this scaffold (.26) wires the route shape + a primitives gallery and
 * a token-aware not-found. Later issues replace the placeholders with real
 * screens via createBrowserRouter children.
 */
import { createBrowserRouter, Link } from "react-router-dom";
import { PrimitivesGallery } from "@/components/dev/primitives-gallery";
import { Button } from "@/components/ui/button";
import { t } from "@/lib/copy";
import { Landing } from "@/screens/landing";
import { Dashboard } from "@/screens/dashboard";
import { RulesManager } from "@/screens/rules-manager";
import { Settings } from "@/screens/settings";
import { Cli } from "@/screens/cli";

function NotFound() {
  return (
    <div className="flex min-h-screen flex-col items-center justify-center gap-3 bg-canvas p-6 text-center">
      <h1 className="text-h2 text-text-primary">
        {t("common.notFound.title")}
      </h1>
      <p className="text-body-sm text-text-tertiary">
        {t("common.notFound.body")}
      </p>
      <Button variant="link" asChild>
        <Link to="/">{t("common.notFound.home")}</Link>
      </Button>
    </div>
  );
}

export const router = createBrowserRouter([
  { path: "/", element: <Landing /> },
  { path: "/d/:token", element: <Dashboard /> },
  { path: "/d/:token/rules", element: <RulesManager /> },
  { path: "/d/:token/settings", element: <Settings /> },
  { path: "/cli", element: <Cli /> },
  // Dev-only route: `import.meta.env.DEV` is a build-time constant, so Vite
  // dead-code-eliminates this branch (and the PrimitivesGallery import with
  // it) from the production bundle — nginx's SPA fallback never has a page
  // to serve here in prod, and /_gallery falls through to NotFound.
  ...(import.meta.env.DEV
    ? [{ path: "/_gallery", element: <PrimitivesGallery /> }]
    : []),
  { path: "*", element: <NotFound /> },
]);
