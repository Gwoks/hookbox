/**
 * Static §5 contract checks that don't need a browser (Node-context Playwright
 * tests, same pattern as no-hex.spec.ts). operator-toolkit F2/AC-9: removing
 * the sub-header's "Local path" chip must NOT touch the wire contract —
 * `path_url` stays a required (non-optional, non-nullable) string on
 * `GET /api/endpoints/{token}`'s detail shape.
 */
import * as fs from "node:fs";
import * as path from "node:path";
import { expect, test } from "@playwright/test";
import { endpointDetailSchema, mockRuleCreateSchema } from "../src/api/schemas";
import { DEFAULT_CATCH_ALL_RULE } from "../src/screens/rules-manager";

function validDetail(): Record<string, unknown> {
  return {
    token: "ab12cd34",
    name: "Checkout API",
    mock_url: "https://ab12cd34.hookbox.test",
    path_url: "http://localhost:8080/m/ab12cd34",
    auto_crud: true,
    target_url: null,
    default_mode: "mock_404",
    latency_ms: 0,
    rate_limit_per_min: 0,
    chaos_pct: 0,
    chaos_mode: "error",
    cors_enabled: false,
    tunnel_active: false,
    created_at: "2026-06-21T12:00:00Z",
    last_hit: null,
    request_count: 3,
  };
}

test("AC-9: path_url is still a required string on endpointDetailSchema", () => {
  const withPathUrl = validDetail();
  expect(endpointDetailSchema.safeParse(withPathUrl).success).toBe(true);

  const { path_url: _omitted, ...withoutPathUrl } = validDetail();
  expect(endpointDetailSchema.safeParse(withoutPathUrl).success).toBe(false);

  expect(
    endpointDetailSchema.safeParse({ ...validDetail(), path_url: null })
      .success,
  ).toBe(false);
});

test("AC-59: F6's default catch-all payload matches the frozen §5.5.7 bytes exactly", () => {
  // A valid MockRuleCreate per the client's own schema.
  expect(mockRuleCreateSchema.safeParse(DEFAULT_CATCH_ALL_RULE).success).toBe(
    true,
  );
  expect(DEFAULT_CATCH_ALL_RULE).toEqual({
    name: "Catch-all (default)",
    priority: 1000,
    enabled: true,
    match: {
      method: "ANY",
      path: "/*",
      headers: {},
      query: {},
      body_conditions: [],
      state_requirements: [],
    },
    response: {
      status_code: 200,
      headers: {},
      content_type: "application/json",
      body_template:
        '{\n  "ok": true,\n  "hookbox": "default catch-all",\n  "message": "Edit this rule in HookBox to return your own response."\n}',
    },
    state_writes: [],
    latency_ms: null,
    rate_limit_per_min: null,
    chaos_mode: null,
    webhook_action: null,
  });
  // priority = 1000 is inside the accepted 0..=100000 band and sorts LAST
  // against every default-priority (100) rule — "lower wins".
  expect(DEFAULT_CATCH_ALL_RULE.priority).toBeGreaterThan(100);
});

test("AC-42/AC-S13: the /s/:code viewer's module graph never reaches src/api/session.ts", () => {
  // A static source check, not a bundler trace: the owner secret lives in
  // localStorage (src/api/session.ts), and this page is same-origin with the
  // dashboard, so any accidental import here would be owner takeover on XSS.
  const files = [
    "src/screens/share-view.tsx",
    "src/screens/share-view/use-shared-feed.ts",
    "src/screens/share-view/row.tsx",
  ];
  for (const relPath of files) {
    const text = fs.readFileSync(path.join(process.cwd(), relPath), "utf-8");
    // Strip line/block comments before scanning for real code references —
    // prose in the file's own header comment legitimately says "session"
    // (e.g. "no account, no session" / "MUST NOT reach src/api/session.ts").
    const code = text
      .replace(/\/\*[\s\S]*?\*\//g, "")
      .replace(/^\s*\/\/.*$/gm, "");
    expect(text, `${relPath} must not import session.ts`).not.toMatch(
      /from\s+['"]@\/api\/session['"]/,
    );
    expect(code, `${relPath} must not reference \`session\` in code`).not.toMatch(
      /\bsession\b/,
    );
    expect(code, `${relPath} must not import AppShell`).not.toContain("AppShell");
    expect(code, `${relPath} must not import useSession`).not.toContain("useSession");
  }
});
