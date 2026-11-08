/**
 * AC-D11 / AC-D23 — the "no raw hex outside the token file" hard rule
 * (design.md §9). A static grep over components, screens, the theme module
 * and the router must return zero literal hex colors; all color flows through
 * Tailwind token classes / CSS vars. Token definitions live in globals.css /
 * tailwind.config.ts, which are intentionally excluded — and so is src/lib
 * (copy.ts legitimately contains strings like "#421").
 */
import { execSync } from "node:child_process";
import { expect, test } from "@playwright/test";

const SCANNED = ["src/components/", "src/screens/", "src/theme/", "src/router.tsx"];

test("AC-D11/D23: no raw hex colors in components, screens, theme or router", () => {
  // grep exits 1 on no matches — that's the pass; `|| true` keeps stdout.
  const hits = execSync(`grep -rnE '#[0-9a-fA-F]{3,6}' ${SCANNED.join(" ")} || true`, {
    cwd: process.cwd(),
    encoding: "utf8",
  }).trim();
  expect(hits, `Raw hex found (use tokens):\n${hits}`).toBe("");
});
