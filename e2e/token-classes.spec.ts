/**
 * Token-class hygiene (hookbox-a73 S4). Static, Node-only checks over src/
 * for Tailwind classes that silently generate NO CSS or bypass the token
 * system — none of them fail the build, so they must fail here:
 *
 *  - `bg-subtle` / `bg-active` / `hover:bg-hover` — the colors are
 *    `surface.subtle|active|hover`; the bare names do not exist.
 *  - alpha modifiers on semantic colors (`border-danger-fg/40`) — Tailwind
 *    cannot apply an alpha to a `var(--…)` color, so no CSS is emitted; add a
 *    semantic token (e.g. `border-danger-border`) instead.
 *  - arbitrary radii (`rounded-[4px]`) — use the xs/sm/md/pill scale.
 *  - gradients — DESIGN.md is strictly flat; only the slider's filled track
 *    and the skeleton shimmer (globals.css) are allowed one.
 */
import { readFileSync } from "node:fs";
import path from "node:path";
import { expect, test } from "@playwright/test";
import { SRC, rel, sourceFiles } from "./support/import-graph";

const GRADIENT_ALLOWED = new Set(["src/components/ui/slider.tsx"]);

const RULES: { name: string; pattern: RegExp; applies?: (file: string) => boolean }[] = [
  {
    name: "nonexistent bg/text/border-(subtle|active|hover) — use the surface-* colors",
    pattern: /(?<![\w-])(?:bg|text|border)-(?:subtle|active|hover)\b/g,
  },
  {
    name: "alpha modifier on a semantic color — add a token instead",
    pattern: /(?<![\w-])(?:bg|text|border|ring|outline|divide|fill|stroke|shadow|decoration|placeholder)-[a-z][\w-]*-(?:fg|bg)\/\d+/g,
  },
  {
    name: "arbitrary border radius — use rounded-xs|sm|md|pill",
    pattern: /rounded(?:-[a-z]{1,2})?-\[/g,
  },
  {
    name: "gradient — DESIGN.md is strictly flat",
    pattern: /gradient/gi,
    applies: (file) => !GRADIENT_ALLOWED.has(file),
  },
];

function findViolations(): string[] {
  return sourceFiles(SRC).flatMap((abs) => {
    const file = rel(abs);
    const lines = readFileSync(abs, "utf8").split("\n");
    return RULES.filter((rule) => rule.applies?.(file) ?? true).flatMap((rule) =>
      lines.flatMap((line, i) =>
        [...line.matchAll(rule.pattern)].map(
          (m) => `${file}:${i + 1}  ${m[0]}  (${rule.name})`,
        ),
      ),
    );
  });
}

test("src/ uses only token classes that generate CSS", () => {
  const violations = findViolations();
  expect(violations, violations.join("\n")).toEqual([]);
});

test("the scanner actually sees the source tree", () => {
  const files = sourceFiles(SRC).map(rel);
  expect(files).toContain(path.join("src", "components", "ui", "button.tsx"));
});
