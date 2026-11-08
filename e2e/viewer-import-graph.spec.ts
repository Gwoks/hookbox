/**
 * AC-S13 — static assertion that the public /s/:code viewer's module graph
 * never reaches src/api/session.ts (defence-in-depth: this page is built
 * entirely from attacker-supplied text with no CSP behind it). Walks every
 * import / re-export edge (including type-only imports and dynamic
 * `import()`) starting at src/screens/share-view.tsx using the TypeScript
 * compiler API, and fails if any path reaches session.ts. Runs entirely in
 * Node — no browser, no built app, no webServer dependency.
 */
import path from "node:path";
import { expect, test } from "@playwright/test";
import { SRC, findImportChain, rel } from "./support/import-graph";

const ENTRY = path.join(SRC, "screens", "share-view.tsx");
const FORBIDDEN = path.join(SRC, "api", "session.ts");

test("AC-S13: the /s/:code viewer's module graph never imports src/api/session.ts", () => {
  const chain = findImportChain(ENTRY, FORBIDDEN);
  const relChain = chain?.map(rel).join(" -> ");
  expect(chain, `import chain reaching session.ts:\n${relChain}`).toBeNull();
});
