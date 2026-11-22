/**
 * Playwright config — the FE half of the build gate (PRD §7, AC-55). Runs the
 * e2e suite (e2e/) over the built SPA served by `vite preview`, with every §5
 * backend call intercepted by the in-spec mock (e2e/mock-backend.ts) so the
 * suite is self-contained (no Rust backend needed; "mock/real §5 backend"). A
 * second project drives prefers-reduced-motion for AC-D16.
 */
import { defineConfig, devices } from "@playwright/test";

// E2E_PORT lets parallel worktrees run the suite without sharing (and
// silently reusing) one preview server.
const PORT = Number(process.env.E2E_PORT ?? 4173);
const BASE_URL = `http://localhost:${PORT}`;
/** Cloudflare's documented always-pass Turnstile test site key. The build
 * env below overrides any real key in a developer's root .env (Vite never
 * lets .env override an already-set process variable), and e2e/mock-backend.ts
 * stubs the challenge script, so e2e never reaches Cloudflare. */
const E2E_TURNSTILE_SITE_KEY = "1x00000000000000000000AA";

/** process.env (minus unset entries) + the e2e build overrides. */
function webServerEnv(): Record<string, string> {
  const inherited = Object.fromEntries(
    Object.entries(process.env).filter((entry): entry is [string, string] => entry[1] !== undefined),
  );
  return { ...inherited, VITE_TURNSTILE_SITE_KEY: E2E_TURNSTILE_SITE_KEY };
}

export default defineConfig({
  testDir: "./e2e",
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  reporter: process.env.CI ? "github" : "list",
  use: {
    baseURL: BASE_URL,
    trace: "on-first-retry",
  },
  projects: [
    {
      name: "chromium",
      use: { ...devices["Desktop Chrome"] },
      // Reduced-motion assertions run in their own project below.
      testIgnore: /reduced-motion\.spec\.ts/,
    },
    {
      name: "reduced-motion",
      use: { ...devices["Desktop Chrome"], reducedMotion: "reduce" },
      testMatch: /reduced-motion\.spec\.ts/,
    },
  ],
  webServer: {
    // Build once, then serve dist/ — the same artifact the Rust spa.rs ships.
    command: `pnpm build && pnpm preview --port ${PORT} --strictPort`,
    env: webServerEnv(),
    url: BASE_URL,
    reuseExistingServer: !process.env.CI,
    timeout: 180_000,
  },
});
