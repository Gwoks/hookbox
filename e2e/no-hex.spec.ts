/**
 * AC-D11 / AC-D23 — the "no raw hex in components" hard rule (design.md §9). A
 * static grep over src/components/** must return zero literal hex colors; all
 * color flows through Tailwind token classes / CSS vars. Token definitions live
 * in globals.css / tailwind.config.ts, which are intentionally excluded.
 */
import { execSync } from "node:child_process";
import { expect, test } from "@playwright/test";

test("AC-D11/D23: no raw hex colors under src/components/**", () => {
  let hits = "";
  try {
    // grep exits 1 (no matches) → that's the pass; capture stdout otherwise.
    hits = execSync(`grep -rnE '#[0-9a-fA-F]{3,6}' src/components/ || true`, {
      cwd: process.cwd(),
      encoding: "utf8",
    }).trim();
  } catch {
    hits = "";
  }
  expect(hits, `Raw hex found in components (use tokens):\n${hits}`).toBe("");
});
