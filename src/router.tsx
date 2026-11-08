/**
 * SPA route tree (PRD §3 surfaces). URLs: landing / session gate (/), the
 * owner screens under /d/:token (dashboard, rules, settings — gated by the
 * session, then by the endpoint load in OwnerLayout), the public CLI page
 * (/cli), the public read-only share viewer (/s/:code) and a NotFound.
 */
import { createBrowserRouter } from "react-router-dom";
import { NotFound } from "@/app/not-found";
import { OwnerLayout } from "@/app/owner-layout";
import { PrimitivesGallery } from "@/components/dev/primitives-gallery";
import { RequireSession } from "@/components/owner/require-session";
import { Landing } from "@/screens/landing";
import { Dashboard } from "@/screens/dashboard";
import { RulesManager } from "@/screens/rules";
import { Settings } from "@/screens/settings";
import { Cli } from "@/screens/cli";
import { ShareView } from "@/screens/share-view";

export const router = createBrowserRouter([
  { path: "/", element: <Landing /> },
  // Owner screens render only once the session gate knows the tab is signed
  // in (cookie identity, hookbox-a73.3; signed-out bounces to /), then once
  // OwnerLayout has loaded the endpoint they all share.
  {
    path: "/d/:token",
    element: (
      <RequireSession>
        <OwnerLayout />
      </RequireSession>
    ),
    children: [
      { index: true, element: <Dashboard /> },
      { path: "rules", element: <RulesManager /> },
      { path: "settings", element: <Settings /> },
    ],
  },
  { path: "/cli", element: <Cli /> },
  // Public, unauthenticated read-only share viewer (operator-toolkit F4).
  // Must never redirect to / or touch the session (AC-41) — registered
  // before the `*` fallback so /s/<code> resolves here.
  { path: "/s/:code", element: <ShareView /> },
  // Dev-only route: `import.meta.env.DEV` is a build-time constant, so Vite
  // dead-code-eliminates this branch (and the PrimitivesGallery import with
  // it) from the production bundle — /_gallery falls through to NotFound.
  ...(import.meta.env.DEV
    ? [{ path: "/_gallery", element: <PrimitivesGallery /> }]
    : []),
  { path: "*", element: <NotFound /> },
]);
