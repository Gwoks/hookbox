/**
 * Landing e2e (AC-J1, AC-D22, hookbox-a73.3). No email: "Get my endpoint"
 * mints an anonymous owner whose recovery key lives in an HttpOnly cookie
 * (modelled by the mock's `store.authed`); "I have a recovery key" restores
 * one; a browser still holding the pre-cookie localStorage secret is moved
 * onto the cookie silently. There is no "welcome back" anywhere (AC-D22).
 */
import { expect, test } from "@playwright/test";
import {
  installMockBackend,
  LEGACY_STORAGE_KEYS,
  OWNER_ID,
  SECRET,
  TOKEN,
} from "./mock-backend";

function legacyValues(page: import("@playwright/test").Page) {
  return page.evaluate(
    (keys) => keys.map((key) => localStorage.getItem(key)),
    [...LEGACY_STORAGE_KEYS],
  );
}

test.describe("landing gate (AC-J1)", () => {
  test("Get my endpoint → mints a session, lands on the dashboard and nudges to save the key", async ({
    page,
  }) => {
    const store = await installMockBackend(page);
    await page.goto("/");

    await expect(page.getByLabel(/email/i)).toHaveCount(0);
    await page.getByRole("button", { name: "Get my endpoint" }).click();

    await expect(page).toHaveURL(new RegExp(`/d/${TOKEN}$`));
    expect(store.authed).toBe(true);
    const nudge = page.getByRole("status").filter({ hasText: "Save your recovery key" });
    await expect(nudge).toBeVisible();
    await expect(nudge.getByRole("link", { name: "Go to recovery key" })).toHaveAttribute(
      "href",
      `/d/${TOKEN}/settings#recovery-key`,
    );
    await nudge.getByRole("button", { name: "Dismiss" }).click();
    await expect(page.getByText("Save your recovery key")).toHaveCount(0);
  });

  test("a valid recovery key restores access and opens the dashboard", async ({ page }) => {
    const store = await installMockBackend(page);
    await page.goto("/");

    await page.getByRole("button", { name: "I have a recovery key" }).click();
    const field = page.getByLabel("Recovery key");
    await expect(field).toHaveAttribute("type", "password");
    await expect(field).toHaveAttribute("autocomplete", "off");
    await expect(field).toHaveAttribute("spellcheck", "false");
    await field.fill(SECRET);
    await page.getByRole("button", { name: "Restore access" }).click();

    await expect(page).toHaveURL(new RegExp(`/d/${TOKEN}$`));
    expect(store.authed).toBe(true);
    // A restore is not a fresh owner: no save-your-key nudge.
    await expect(page.getByText("Save your recovery key")).toHaveCount(0);
  });

  test("an unknown recovery key shows a field error and stays on the landing", async ({
    page,
  }) => {
    const store = await installMockBackend(page);
    await page.goto("/");

    await page.getByRole("button", { name: "I have a recovery key" }).click();
    await page.getByLabel("Recovery key").fill("not-a-real-key");
    await page.getByRole("button", { name: "Restore access" }).click();

    await expect(page.getByText("That recovery key didn't work. Check it and try again.")).toBeVisible();
    await expect(page.getByLabel("Recovery key")).toHaveAttribute("aria-invalid", "true");
    await expect(page).toHaveURL(/\/$/);
    expect(store.authed).toBe(false);
  });

  test("a returning signed-in browser goes straight to its primary endpoint", async ({
    page,
  }) => {
    await installMockBackend(page, { authed: true });
    await page.goto("/");
    await expect(page).toHaveURL(new RegExp(`/d/${TOKEN}$`));
  });

  test("a legacy localStorage secret is migrated to the cookie and all three keys are removed", async ({
    page,
  }) => {
    const restoreBodies: unknown[] = [];
    page.on("request", (request) => {
      if (request.url().endsWith("/api/session/restore")) {
        restoreBodies.push(request.postDataJSON());
      }
    });
    const store = await installMockBackend(page, { legacySecret: SECRET });
    await page.goto("/");

    await expect(page).toHaveURL(new RegExp(`/d/${TOKEN}$`));
    expect(store.authed).toBe(true);
    expect(restoreBodies).toEqual([{ key: SECRET }]);
    expect(await legacyValues(page)).toEqual([null, null, null]);
  });

  test("a rejected legacy secret is removed and the landing is shown", async ({ page }) => {
    await installMockBackend(page, { legacySecret: "stale_secret_from_old_email_login" });
    await page.goto("/");

    await expect(page.getByRole("button", { name: "Get my endpoint" })).toBeVisible();
    expect(await legacyValues(page)).toEqual([null, null, null]);
  });

  test("the account menu shows the anonymous owner id, never an email", async ({ page }) => {
    await installMockBackend(page, { authed: true });
    await page.goto(`/d/${TOKEN}`);
    await page.getByRole("button", { name: "Account" }).click();
    await expect(page.getByText(`Anonymous owner · ${OWNER_ID.slice(0, 8)}`)).toBeVisible();
    await expect(page.getByRole("menu")).not.toContainText("@");
  });

  test("AC-D22: no 'welcome back' string anywhere on the gate", async ({
    page,
  }) => {
    await installMockBackend(page, { authed: false });
    await page.goto("/");
    await expect(page.getByRole("button", { name: "Get my endpoint" })).toBeVisible();
    await expect(page.getByText(/welcome back/i)).toHaveCount(0);
  });
});

test.describe("landing page (FE-P5)", () => {
  test("the hero sets one verb on a peach pill and pairs the one primary with a tint secondary", async ({
    page,
  }) => {
    await installMockBackend(page, { authed: false });
    await page.goto("/");
    const h1 = page.getByRole("heading", { level: 1 });
    await expect(h1).toHaveText("Mock, intercept, and watch HTTP — live.");
    await expect(h1.getByText("intercept", { exact: true })).toHaveClass(/rounded-pill/);
    await expect(h1.getByText("intercept", { exact: true })).toHaveClass(/bg-card-peach/);

    await expect(
      page.getByRole("button", { name: "Get my endpoint", exact: true }),
    ).toHaveClass(/bg-accent-fill/);
    await expect(
      page.getByRole("button", { name: "I have a recovery key", exact: true }),
    ).toHaveClass(/bg-accent-subtle-bg/);
    // DESIGN.md: one filled blue action per screen.
    await expect(page.locator('[class~="bg-accent-fill"]')).toHaveCount(1);
  });

  test("the header links to the feature grid, the CLI and the call to action", async ({
    page,
  }) => {
    await installMockBackend(page, { authed: false });
    await page.goto("/");
    const banner = page.getByRole("banner");
    await banner.getByRole("link", { name: "How it works", exact: true }).click();
    await expect(page.getByRole("heading", { level: 2, name: "One URL. Every request on the record." })).toBeInViewport();
    await expect(banner.getByRole("link", { name: "CLI", exact: true })).toHaveAttribute("href", "/cli");
    await banner.getByRole("link", { name: "Get started", exact: true }).click();
    await expect(page.getByRole("button", { name: "Get my endpoint", exact: true })).toBeInViewport();
    // The product mockup is a picture, not a second (inert) dashboard.
    await expect(page.getByRole("option")).toHaveCount(0);
  });
});
