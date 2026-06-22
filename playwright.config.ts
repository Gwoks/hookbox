/**
 * Playwright config — the FE half of the build gate (PRD §7, AC-55). Runs the
 * e2e suite (e2e/) over the built SPA served by `vite preview`, with every §5
 * backend call intercepted by the in-spec mock (e2e/mock-backend.ts) so the
 * suite is self-contained (no Rust backend needed; "mock/real §5 backend"). A
 * second project drives prefers-reduced-motion for AC-D16.
 */
import { defineConfig, devices } from "@playwright/test";

const PORT = 4173;
const BASE_URL = `http://localhost:${PORT}`;

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
    url: BASE_URL,
    reuseExistingServer: !process.env.CI,
    timeout: 180_000,
  },
});
