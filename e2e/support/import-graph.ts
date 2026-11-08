/**
 * Shared static import-graph helpers for the Node-only guard specs
 * (viewer-import-graph, import-boundaries). Parses every import / re-export /
 * dynamic `import()` edge — including type-only imports — with the TypeScript
 * compiler API and resolves `@/` aliases and relative specifiers to absolute
 * paths under src/. Bare package imports resolve to null.
 */
import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import path from "node:path";
import ts from "typescript";

export const ROOT = process.cwd();
export const SRC = path.join(ROOT, "src");
const RESOLVABLE_EXTENSIONS = [".ts", ".tsx", ".js", ".jsx"];

function isFile(candidate: string): boolean {
  return existsSync(candidate) && statSync(candidate).isFile();
}

/** Resolve a relative (`./foo`) or aliased (`@/foo`) specifier to an absolute
 * file path under src/, or null for bare package imports. */
export function resolveModule(fromFile: string, specifier: string): string | null {
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
export function extractSpecifiers(filePath: string): string[] {
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

/** Resolved local (project-source) files a file imports directly. */
export function directImports(filePath: string): string[] {
  return extractSpecifiers(filePath)
    .map((specifier) => resolveModule(filePath, specifier))
    .filter((resolved): resolved is string => resolved !== null);
}

/** BFS over the local import graph from `entry`. Returns the first chain of
 * file paths reaching `forbidden`, or null if unreachable. */
export function findImportChain(entry: string, forbidden: string): string[] | null {
  const visited = new Set<string>([entry]);
  const queue: string[][] = [[entry]];
  while (queue.length > 0) {
    const chain = queue.shift() as string[];
    const file = chain[chain.length - 1];
    if (file === forbidden) return chain;
    for (const resolved of directImports(file)) {
      if (!visited.has(resolved)) {
        visited.add(resolved);
        queue.push([...chain, resolved]);
      }
    }
  }
  return null;
}

/** Every .ts/.tsx source file under `dir` (recursive), excluding unit tests. */
export function sourceFiles(dir: string): string[] {
  if (!existsSync(dir)) return [];
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) return sourceFiles(full);
    return /\.tsx?$/.test(entry.name) && !/\.test\.tsx?$/.test(entry.name) ? [full] : [];
  });
}

export const rel = (file: string): string => path.relative(ROOT, file);
