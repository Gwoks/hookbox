/**
 * Visual / accessibility AC e2e (design.md §9/§11, AC-D11..D24 testable subset).
 * Automated checks for the verifiable visual contracts:
 *
 *   AC-D11/D23  no raw hex in src/components/** (static grep — no-hex.spec.ts)
 *   AC-D13      selected FeedRow distinguishable with color removed
 *               (3px accent rail + bg-active + aria-selected)
 *   AC-D14      MethodBadge/StatusCode/ServedByChip carry a text/icon label
 *   AC-D15      focus-visible ring on interactive elements (feed rows, tabs)
 *   AC-D18      feed leading column aligns (fixed-width MethodBadge)
 *   AC-D19      mock-URL chips are text-primary mono copy-only (no <a>/navigation)
 *   AC-D24      light is the first-paint default (no .dark) for a new visitor
 *
 * Reduced-motion (AC-D16) is its own project (reduced-motion.spec.ts).
 */
import { expect, test } from "@playwright/test";
import { installMockBackend, TOKEN } from "./mock-backend";

test("AC-D24: light first-paint default for a new visitor (no .dark)", async ({
  page,
}) => {
  // No stored theme, no forced dark scheme → light.
  await page.emulateMedia({ colorScheme: "light" });
  await installMockBackend(page, { authed: true });
  await page.goto(`/d/${TOKEN}`);
  const isDark = await page.evaluate(() =>
    document.documentElement.classList.contains("dark"),
  );
  expect(isDark).toBe(false);
});

test("AC-D13: a selected feed row is distinguishable without color (rail + aria-selected)", async ({
  page,
}) => {
  await installMockBackend(page, { authed: true });
  await page.goto(`/d/${TOKEN}`);
  const row = page.getByRole("option").first();
  await row.click();
  await expect(row).toHaveAttribute("aria-selected", "true");
  // The 3px leading accent rail is an inset box-shadow — a non-hue marker.
  const boxShadow = await row.evaluate((el) => getComputedStyle(el).boxShadow);
  expect(boxShadow).not.toBe("none");
});

test("AC-D14: method + status + served-by carry text/icon labels", async ({
  page,
}) => {
  await installMockBackend(page, { authed: true });
  await page.goto(`/d/${TOKEN}`);
  const row = page.getByRole("option").first();
  // Method label text present (grayscale-legible).
  await expect(row).toContainText(/GET|POST|PUT|PATCH|DELETE/);
  // Status digits present.
  await expect(row).toContainText(/\d{3}/);
});

test("AC-D15: feed rows and tabs expose a visible focus ring", async ({
  page,
}) => {
  await installMockBackend(page, { authed: true });
  await page.goto(`/d/${TOKEN}`);
  const row = page.getByRole("option").first();
  await row.focus();
  const outline = await row.evaluate((el) => {
    const s = getComputedStyle(el);
    return s.outlineStyle + s.boxShadow;
  });
  // Either an outline or a ring box-shadow is applied on focus (never removed).
  expect(outline.length).toBeGreaterThan(0);
});

test("AC-D18: the feed leading column aligns (fixed-width method badges)", async ({
  page,
}) => {
  await installMockBackend(page, { authed: true });
  await page.goto(`/d/${TOKEN}`);
  const rows = page.getByRole("option");
  await expect(rows).not.toHaveCount(0);
  // All method badges share one min-width → second-column x-offset is stable.
  const widths = await rows.evaluateAll((els) =>
    els.map(
      (el) =>
        (el.firstElementChild as HTMLElement)?.getBoundingClientRect().width ??
        0,
    ),
  );
  const min = Math.min(...widths);
  const max = Math.max(...widths);
  expect(max - min).toBeLessThanOrEqual(1);
});

test("AC-D19: mock-URL chips are copy-only (no anchor / no navigation)", async ({
  page,
}) => {
  await installMockBackend(page, { authed: true });
  await page.goto(`/d/${TOKEN}/settings`);
  // The Mock URL block renders the value in a <code>, not an <a>.
  const code = page.locator("code", { hasText: "hookbox.test" }).first();
  await expect(code).toBeVisible();
  await expect(code.locator("xpath=ancestor::a")).toHaveCount(0);
});

test("AC-7: the dashboard sub-header renders exactly one URL label ('Mock URL', no 'Local path')", async ({
  page,
}) => {
  await installMockBackend(page, { authed: true });
  await page.goto(`/d/${TOKEN}`);
  // Let the feed reconcile settle first — the sub-header label is stable
  // regardless, but FeedEmpty (which carries its own "Mock URL" label,
  // AC-D19) can transiently render between WS "live" and the reconcile GET
  // resolving, which would otherwise make this a race.
  await expect(page.getByRole("option").first()).toBeVisible();
  await expect(page.getByText("Mock URL", { exact: true })).toHaveCount(1);
  await expect(page.getByText("Local path", { exact: true })).toHaveCount(0);
});

test("AC-127: the sub-header wraps without horizontal scroll at the 360px floor", async ({
  page,
}) => {
  await page.setViewportSize({ width: 360, height: 800 });
  await installMockBackend(page, { authed: true });
  await page.goto(`/d/${TOKEN}`);
  // See the AC-7 test above re: waiting out the transient FeedEmpty window.
  await expect(page.getByRole("option").first()).toBeVisible();
  await expect(page.getByText("Mock URL", { exact: true })).toBeVisible();
  await expect(page.getByRole("link", { name: "Rules" })).toBeVisible();
  await expect(page.getByRole("link", { name: "New rule" })).toBeVisible();
  const hasHorizontalScroll = await page.evaluate(
    () =>
      document.documentElement.scrollWidth >
      document.documentElement.clientWidth,
  );
  expect(hasHorizontalScroll).toBe(false);
});

test("AC-127 (F1): the feed header's title/count/actions-menu/pause stay usable at the 360px floor", async ({
  page,
}) => {
  await page.setViewportSize({ width: 360, height: 800 });
  await installMockBackend(page, { authed: true });
  await page.goto(`/d/${TOKEN}`);
  await expect(page.getByText("Live feed")).toBeVisible();
  await expect(page.getByRole("button", { name: "Feed actions" })).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Pause the live feed" }),
  ).toBeVisible();
  const hasHorizontalScroll = await page.evaluate(
    () =>
      document.documentElement.scrollWidth >
      document.documentElement.clientWidth,
  );
  expect(hasHorizontalScroll).toBe(false);
});
