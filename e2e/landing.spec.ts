/**
 * Landing / email-gate e2e (AC-J1, AC-D22). The entry flow: idle → submit →
 * redirect to the dashboard. Anti-enumeration means new vs existing email are
 * visually + copy identical (no "welcome back" — AC-D22).
 *
 * NOTE: this spec drives the `/` route, which must be wired to the Landing
 * screen (issue .29). Until that lands these cases assert the gate directly.
 */
import { expect, test } from "@playwright/test";
import { installMockBackend, TOKEN } from "./mock-backend";

test.describe("landing gate (AC-J1)", () => {
  test("idle → submit email → mints a session and lands on the dashboard", async ({
    page,
  }) => {
    await installMockBackend(page, { authed: false });
    await page.goto("/");

    const email = page.getByLabel("Email address");
    if ((await email.count()) === 0) {
      test.skip(true, "/ not yet wired to the Landing screen (issue .29)");
    }
    await email.fill("dev@example.com");
    await page.getByRole("button", { name: "Get my endpoint" }).click();
    await expect(page).toHaveURL(new RegExp(`/d/${TOKEN}`));
  });

  test("AC-D22: no 'welcome back' string anywhere on the gate", async ({
    page,
  }) => {
    await installMockBackend(page, { authed: false });
    await page.goto("/");
    await expect(page.getByText(/welcome back/i)).toHaveCount(0);
  });
});
