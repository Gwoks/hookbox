/**
 * Folder import boundaries (hookbox-a73). Shared presentational folders must
 * stay safe for the credential-free /s/:code viewer, so they may only reach
 * dependency-free API modules (`@/api/errors`, `@/api/schemas`, `@/api/http`,
 * `@/api/public-client`) — never the owner client, the session store, the
 * `@/api` barrel or the live feed.
 *
 * KNOWN_OFFENDERS is a ratchet: entries are removed as files move to
 * src/components/owner/ or switch imports. A listed file that no longer
 * violates fails the spec, so the allowlist can only shrink.
 */
import path from "node:path";
import { expect, test } from "@playwright/test";
import { SRC, directImports, rel, sourceFiles } from "./support/import-graph";

const VIEWER_SAFE_DIRS = ["components/ui", "components/hookbox", "components/layout"];
const OWNER_ONLY = [
  path.join(SRC, "api", "index.ts"),
  path.join(SRC, "api", "client.ts"),
  path.join(SRC, "api", "session.ts"),
  path.join(SRC, "api", "bootstrap.ts"),
  path.join(SRC, "api", "use-session.ts"),
];
const FEED_DIR = path.join(SRC, "feed") + path.sep;

const KNOWN_OFFENDERS = new Set([
  "src/components/hookbox/app-shell.tsx",
  "src/components/hookbox/confirm-dialog.tsx",
  "src/components/hookbox/share-dialog.tsx",
]);

function violations(file: string): string[] {
  return directImports(file)
    .filter((target) => OWNER_ONLY.includes(target) || target.startsWith(FEED_DIR))
    .map(rel);
}

test("viewer-safe component folders never import owner-only modules", () => {
  const files = VIEWER_SAFE_DIRS.flatMap((dir) => sourceFiles(path.join(SRC, dir)));
  const found = files
    .map((file) => ({ file: rel(file), bad: violations(file) }))
    .filter(({ bad }) => bad.length > 0);
  const unexpected = found.filter(({ file }) => !KNOWN_OFFENDERS.has(file));
  expect(unexpected, "new owner-only imports in viewer-safe folders").toEqual([]);
  const fixed = [...KNOWN_OFFENDERS].filter((f) => !found.some(({ file }) => file === f));
  expect(fixed, "remove these from KNOWN_OFFENDERS — they are clean now").toEqual([]);
});

test("generic ui primitives never import domain components or any API module", () => {
  const files = sourceFiles(path.join(SRC, "components", "ui"));
  const forbiddenPrefixes = [
    path.join(SRC, "api") + path.sep,
    path.join(SRC, "components", "hookbox") + path.sep,
    path.join(SRC, "components", "owner") + path.sep,
    FEED_DIR,
  ];
  const bad = files.flatMap((file) =>
    directImports(file)
      .filter((target) => forbiddenPrefixes.some((p) => target.startsWith(p)))
      .map((target) => `${rel(file)} -> ${rel(target)}`),
  );
  expect(bad).toEqual([]);
});
