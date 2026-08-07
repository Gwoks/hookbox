/**
 * AC-S13 — static assertion that the public /s/:code viewer's module graph
 * never reaches src/api/session.ts (defence-in-depth: this page is built
 * entirely from attacker-supplied text with no CSP behind it). Walks every
 * import / re-export edge (including type-only imports and dynamic
 * `import()`) starting at src/screens/share-view.tsx using the TypeScript
 * compiler API, and fails if any path reaches session.ts. Runs entirely in
 * Node — no browser, no built app, no webServer dependency.
 */
import { existsSync, readFileSync, statSync } from "node:fs";
import path from "node:path";
import ts from "typescript";
import { expect, test } from "@playwright/test";

const ROOT = process.cwd();
const SRC = path.join(ROOT, "src");
const ENTRY = path.join(SRC, "screens", "share-view.tsx");
const FORBIDDEN = path.join(SRC, "api", "session.ts");
const RESOLVABLE_EXTENSIONS = [".ts", ".tsx", ".js", ".jsx"];

function isFile(candidate: string): boolean {
  return existsSync(candidate) && statSync(candidate).isFile();
}

/** Resolves a relative (`./foo`) or aliased (`@/foo`) specifier to an
 * absolute file path under src/. Returns null for bare package imports
 * (react, zod, lucide-react, ...) — those never reach session.ts. */
function resolveModule(fromFile: string, specifier: string): string | null {
  let base: string;
  if (specifier.startsWith("@/")) {
    base = path.join(SRC, specifier.slice(2));
  } else if (specifier.startsWith(".")) {
    base = path.resolve(path.dirname(fromFile), specifier);
  } else {
    return null;
  }
  const candidates = [
    base,
    ...RESOLVABLE_EXTENSIONS.map((ext) => base + ext),
    ...RESOLVABLE_EXTENSIONS.map((ext) => path.join(base, "index" + ext)),
  ];
  return candidates.find(isFile) ?? null;
}

/** Every static import/export/dynamic-import module specifier a file references. */
function extractSpecifiers(filePath: string): string[] {
  const text = readFileSync(filePath, "utf8");
  const sourceFile = ts.createSourceFile(
    filePath,
    text,
    ts.ScriptTarget.Latest,
    /* setParentNodes */ true,
    filePath.endsWith(".tsx") ? ts.ScriptKind.TSX : ts.ScriptKind.TS,
  );
  const specifiers: string[] = [];
  function visit(node: ts.Node) {
    if (
      (ts.isImportDeclaration(node) || ts.isExportDeclaration(node)) &&
      node.moduleSpecifier &&
      ts.isStringLiteral(node.moduleSpecifier)
    ) {
      specifiers.push(node.moduleSpecifier.text);
    } else if (ts.isCallExpression(node) && node.expression.kind === ts.SyntaxKind.ImportKeyword) {
      const [arg] = node.arguments;
      if (arg && ts.isStringLiteral(arg)) specifiers.push(arg.text);
    }
    ts.forEachChild(node, visit);
  }
  visit(sourceFile);
  return specifiers;
}

/** BFS over the local (project-source) import graph from `entry`. Returns the
 * first chain of file paths reaching `forbidden`, or null if unreachable. */
function findImportChain(entry: string, forbidden: string): string[] | null {
  const visited = new Set<string>([entry]);
  const queue: string[][] = [[entry]];
  while (queue.length > 0) {
    const chain = queue.shift() as string[];
    const file = chain[chain.length - 1];
    if (file === forbidden) return chain;
    for (const specifier of extractSpecifiers(file)) {
      const resolved = resolveModule(file, specifier);
      if (resolved && !visited.has(resolved)) {
        visited.add(resolved);
        queue.push([...chain, resolved]);
      }
    }
  }
  return null;
}

test("AC-S13: the /s/:code viewer's module graph never imports src/api/session.ts", () => {
  const chain = findImportChain(ENTRY, FORBIDDEN);
  const relChain = chain?.map((f) => path.relative(ROOT, f)).join(" -> ");
  expect(chain, `import chain reaching session.ts:\n${relChain}`).toBeNull();
});
