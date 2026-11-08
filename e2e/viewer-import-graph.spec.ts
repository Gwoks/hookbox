/**
 * AC-S13 — static assertion that the public /s/:code viewer's module graph
 * never reaches the owner identity layer: src/api/session.ts (session
 * status + cross-tab channel), src/api/bootstrap.ts (the /api/session calls
 * and legacy-secret migration) or src/api/client.ts (the owner client that
 * sends the session cookie + X-HookBox header). Defence-in-depth: this page
 * is built entirely from attacker-supplied text with no CSP behind it.
 * Walks every import / re-export edge (including type-only imports and
 * dynamic `import()`) starting at src/screens/share-view.tsx using the
 * TypeScript compiler API. Runs entirely in Node — no browser, no built app.
 */
import path from "node:path";
import { expect, test } from "@playwright/test";
import { SRC, findImportChain, rel } from "./support/import-graph";

const ENTRY = path.join(SRC, "screens", "share-view.tsx");
const FORBIDDEN = ["session.ts", "bootstrap.ts", "client.ts"].map((file) =>
  path.join(SRC, "api", file),
);

for (const forbidden of FORBIDDEN) {
  test(`AC-S13: the /s/:code viewer's module graph never imports ${rel(forbidden)}`, () => {
    const chain = findImportChain(ENTRY, forbidden);
    const relChain = chain?.map(rel).join(" -> ");
    expect(chain, `import chain reaching ${rel(forbidden)}:\n${relChain}`).toBeNull();
  });
}
