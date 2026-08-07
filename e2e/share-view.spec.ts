/**
 * F4 public /s/:code viewer e2e (operator-toolkit prd.md §4.4, AC-41..45,
 * AC-105..112, AC-S8/AC-S13). Drives the real built SPA against the §5 mock
 * backend as a completely unauthenticated visitor.
 */
import { expect, test } from "@playwright/test";
import {
  DEFAULT_SHARE_CODE,
  installMockBackend,
  makeRequest,
  TOKEN,
} from "./mock-backend";

const SHARE_URL = `/s/${DEFAULT_SHARE_CODE}`;

// Names that must be ABSENT from the viewer — every owner-only affordance
// (AC-43).
const OWNER_NAMES = [
  "Switch endpoint",
  "Account",
  "Sign out",
  "Rules",
  "New rule",
  "Settings",
  "Pause the live feed",
  "Resume the live feed",
  "Feed actions",
  "Clear all",
  "Export CSV",
  "Share",
  "Copy mock URL",
  "Copy local path",
  "Resize feed and inspector",
];

test.describe("F4 public viewer /s/:code", () => {
  test("AC-41: never redirects to / and creates no session", async ({ page }) => {
    await installMockBackend(page, { authed: false });
    await page.goto(SHARE_URL);
    await expect(page).toHaveURL(new RegExp(SHARE_URL.replace(/\//g, "\\/") + "$"));
    const hasSecret = await page.evaluate(() =>
      localStorage.getItem("hookbox-owner-secret"),
    );
    expect(hasSecret).toBeNull();
  });

  test("AC-42/AC-S13: zero requests carry an Authorization header or a cookie", async ({
    page,
  }) => {
    const authHeaders: string[] = [];
    await installMockBackend(page, { authed: false });
    await page.route("**/api/**", async (route) => {
      const headers = route.request().headers();
      if (headers["authorization"]) authHeaders.push(headers["authorization"]);
      await route.fallback();
    });
    await page.goto(SHARE_URL);
    await expect(page.getByText("Shared requests", { exact: true })).toBeVisible();
    const cookies = await page.context().cookies();
    expect(authHeaders).toHaveLength(0);
    expect(cookies).toHaveLength(0);
  });

  test("AC-45/AC-S8: zero WebSocket/EventSource connections open", async ({ page }) => {
    await installMockBackend(page, { authed: false });
    // Registered AFTER installMockBackend's own init script, so this runs
    // second and is the constructor any page code actually observes.
    await page.addInitScript(() => {
      // @ts-expect-error test hook
      window.__liveAttempts = 0;
      const track = function track() {
        // @ts-expect-error test hook
        window.__liveAttempts++;
      };
      Object.defineProperty(window, "WebSocket", { value: track, writable: true });
      Object.defineProperty(window, "EventSource", { value: track, writable: true });
    });
    await page.goto(SHARE_URL);
    await expect(page.getByText("Shared requests", { exact: true })).toBeVisible();
    await page.waitForTimeout(500);
    const attempts = await page.evaluate(() => (window as unknown as { __liveAttempts: number }).__liveAttempts);
    expect(attempts).toBe(0);
  });

  test("AC-107: standing banner, Read-only chip, static h1, and a footer", async ({
    page,
  }) => {
    await installMockBackend(page, { authed: false });
    await page.goto(SHARE_URL);
    await expect(page.getByRole("heading", { name: "Shared requests", level: 1 })).toBeVisible();
    await expect(page.getByText("Read-only shared view")).toBeVisible();
    await expect(page.getByText("Read-only", { exact: true })).toBeVisible();
    await expect(page.locator("footer")).toContainText("Served by HookBox");
  });

  test("AC-107: the endpoint name never occupies the h1 — it renders only in the subject line", async ({
    page,
  }) => {
    await installMockBackend(page, {
      authed: false,
      endpoint: { name: "Checkout API" },
    });
    await page.goto(SHARE_URL);
    await expect(page.getByRole("heading", { level: 1 })).toHaveText("Shared requests");
    await expect(page.getByText("Endpoint: Checkout API")).toBeVisible();
  });

  test("AC-43: no owner-voiced accessible name renders anywhere on the page", async ({
    page,
  }) => {
    await installMockBackend(page, { authed: false });
    await page.goto(SHARE_URL);
    await expect(page.getByText("Shared requests", { exact: true })).toBeVisible();
    for (const name of OWNER_NAMES) {
      await expect(page.getByRole("button", { name })).toHaveCount(0);
      await expect(page.getByRole("link", { name })).toHaveCount(0);
    }
    const html = await page.content();
    expect(html).not.toContain(TOKEN);
    expect(html.toLowerCase()).not.toContain("mock_url");
  });

  test("AC-109: zero accent-filled controls and no dashboard chrome", async ({ page }) => {
    await installMockBackend(page, { authed: false });
    await page.goto(SHARE_URL);
    await expect(page.getByText("Shared requests", { exact: true })).toBeVisible();
    const accentFilledCount = await page
      .locator('[class*="bg-accent-fill"]')
      .count();
    expect(accentFilledCount).toBe(0);
    await expect(page.locator('[role="separator"]')).toHaveCount(0); // no SplitPane
    await expect(page.getByRole("listbox")).toHaveCount(0); // no FeedRow listbox
  });

  test("AC-110: no horizontal scroll at the 360px floor", async ({ page }) => {
    await page.setViewportSize({ width: 360, height: 800 });
    await installMockBackend(page, { authed: false });
    await page.goto(SHARE_URL);
    await expect(page.getByText("Shared requests", { exact: true })).toBeVisible();
    const hasHorizontalScroll = await page.evaluate(
      () =>
        document.documentElement.scrollWidth >
        document.documentElement.clientWidth,
    );
    expect(hasHorizontalScroll).toBe(false);
  });

  test("AC-44/108: the empty state covers both causes", async ({ page }) => {
    await installMockBackend(page, { authed: false, requests: [] });
    await page.goto(SHARE_URL);
    await expect(page.getByText("No requests to show")).toBeVisible();
    await expect(page.getByText(/rolled off/)).toBeVisible();
  });

  test("AC-44/65: rows are disclosure buttons (aria-expanded/aria-controls) inside a role=region well, not role=option", async ({
    page,
  }) => {
    await installMockBackend(page, { authed: false });
    await page.goto(SHARE_URL);
    // Locate by the stable `id` (the accessible name flips expand<->collapse
    // wording on click, so a name-based locator would stop matching).
    const row = page.locator('button[id^="share-row-"]').first();
    await expect(row).toBeVisible();
    await expect(row).toHaveAttribute("aria-expanded", "false");
    await row.click();
    await expect(row).toHaveAttribute("aria-expanded", "true");
    await expect(page.locator('[role="region"][id^="share-panel-"]')).toBeVisible();
    await expect(page.getByRole("option")).toHaveCount(0);
    // AC-112: selecting a row does not change the URL.
    await expect(page).toHaveURL(new RegExp(SHARE_URL.replace(/\//g, "\\/") + "$"));
  });

  test("AC-44/106: a detail 404 renders 'gone', stays open, and the rest of the list still works", async ({
    page,
  }) => {
    await installMockBackend(page, { authed: false, shareDetailStatus: 404 });
    await page.goto(SHARE_URL);
    const row = page.locator('button[id^="share-row-"]').first();
    await row.click();
    await expect(page.getByText("This request is no longer available")).toBeVisible();
    await expect(page.getByText("The rest of the list still works.")).toBeVisible();
    // Still expanded/open, not collapsed.
    await expect(row).toHaveAttribute("aria-expanded", "true");
  });

  test("AC-44/106: a detail 5xx renders a distinct, retryable error (not 'gone')", async ({
    page,
  }) => {
    await installMockBackend(page, { authed: false, shareDetailStatus: 500 });
    await page.goto(SHARE_URL);
    const row = page
      .getByRole("button", { name: /Show detail for POST \/orders/ })
      .first();
    await row.click();
    await expect(page.getByText("Couldn't load this request")).toBeVisible();
    await expect(page.getByText("This request is no longer available")).toHaveCount(0);
  });

  test("AC-44/105(a): a LIST 404 is terminal — the unavailable page renders and polling stops for good", async ({
    page,
  }) => {
    let requestCount = 0;
    await installMockBackend(page, { authed: false, shareUnavailable: true });
    await page.route(`**/api/share/${DEFAULT_SHARE_CODE}/requests`, async (route) => {
      requestCount++;
      await route.fallback();
    });
    await page.goto(SHARE_URL);
    await expect(page.getByText("This link isn't available")).toBeVisible();
    await expect(page.getByText("Shared requests", { exact: true })).toHaveCount(0);
    const countAfterUnavailable = requestCount;
    await page.waitForTimeout(6000); // > one 5s poll interval
    expect(requestCount).toBe(countAfterUnavailable); // no further polls, ever
  });

  test("AC-112: /s/ with no code falls through to NotFound", async ({ page }) => {
    await installMockBackend(page, { authed: false });
    await page.goto("/s/");
    await expect(page.getByText("That page doesn't exist.")).toBeVisible();
  });

  test("AC-105(b): a 429 pauses for Retry-After, then retries once", async ({ page }) => {
    await installMockBackend(page, {
      authed: false,
      shareRateLimitRetryAfter: 2,
    });
    await page.goto(SHARE_URL);
    await expect(page.getByText("Too many requests")).toBeVisible();
    await expect(page.getByText(/retries in \d+s/)).toBeVisible();
  });

  test("AC-44: offline suspends polling and shows the offline banner", async ({
    page,
    context,
  }) => {
    await installMockBackend(page, { authed: false });
    await page.goto(SHARE_URL);
    await expect(page.getByText("Shared requests", { exact: true })).toBeVisible();
    await context.setOffline(true);
    await page.evaluate(() => window.dispatchEvent(new Event("offline")));
    await expect(page.getByText("You're offline")).toBeVisible();
    await context.setOffline(false);
  });
});
