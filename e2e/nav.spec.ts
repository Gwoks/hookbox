/**
 * Owner chrome & routing e2e (FE-P4): the top nav's aria-current, the
 * mobile "Menu" sheet, "New endpoint", the OwnerLayout load-failure frames
 * and NotFound.
 */
import { expect, test, type Page } from "@playwright/test";
import { installMockBackend, NEW_TOKEN, TOKEN } from "./mock-backend";

const desktopNav = (page: Page) =>
  page.getByRole("banner").getByRole("navigation", { name: "Main" });

test.describe("owner top nav", () => {
  test("aria-current follows the route and New rule hides on the rules page", async ({
    page,
  }) => {
    await installMockBackend(page, { authed: true });
    await page.goto(`/d/${TOKEN}`);
    const nav = desktopNav(page);
    const requests = nav.getByRole("link", { name: "Requests", exact: true });
    const rules = nav.getByRole("link", { name: "Rules", exact: true });
    const settings = nav.getByRole("link", { name: "Settings", exact: true });
    const newRule = page
      .getByRole("banner")
      .getByRole("link", { name: "New rule", exact: true });

    await expect(requests).toHaveAttribute("aria-current", "page");
    await expect(newRule).toBeVisible();

    await rules.click();
    await expect(page).toHaveURL(new RegExp(`/d/${TOKEN}/rules$`));
    await expect(rules).toHaveAttribute("aria-current", "page");
    await expect(requests).not.toHaveAttribute("aria-current", "page");
    // The rules page header owns the primary "New rule" there.
    await expect(newRule).toHaveCount(0);

    await settings.click();
    await expect(page).toHaveURL(new RegExp(`/d/${TOKEN}/settings$`));
    await expect(settings).toHaveAttribute("aria-current", "page");
    await expect(newRule).toBeVisible();
  });

  test("the mobile Menu sheet opens, closes on Escape and returns focus", async ({
    page,
  }) => {
    await page.setViewportSize({ width: 360, height: 800 });
    await installMockBackend(page, { authed: true });
    await page.goto(`/d/${TOKEN}`);
    const trigger = page.getByRole("button", { name: "Open menu", exact: true });
    await trigger.click();
    const menu = page.getByRole("dialog", { name: "Menu" });
    await expect(menu).toBeVisible();
    await expect(
      menu.getByRole("link", { name: "Requests", exact: true }),
    ).toHaveAttribute("aria-current", "page");
    await expect(menu.getByRole("button", { name: "Sign out", exact: true })).toBeVisible();

    await page.keyboard.press("Escape");
    await expect(menu).toHaveCount(0);
    await expect(trigger).toBeFocused();

    // A link inside the sheet navigates and closes it.
    await trigger.click();
    await menu.getByRole("link", { name: "Settings", exact: true }).click();
    await expect(page).toHaveURL(new RegExp(`/d/${TOKEN}/settings$`));
    await expect(menu).toHaveCount(0);
  });

  test("New endpoint creates one and opens it", async ({ page }) => {
    const creates: string[] = [];
    page.on("request", (request) => {
      if (
        request.method() === "POST" &&
        new URL(request.url()).pathname === "/api/endpoints"
      ) {
        creates.push(request.url());
      }
    });
    await installMockBackend(page, { authed: true });
    await page.goto(`/d/${TOKEN}`);
    await page.getByRole("button", { name: "Switch endpoint", exact: true }).click();
    await page.getByRole("menuitem", { name: "Create a new endpoint" }).click();

    await expect(page).toHaveURL(new RegExp(`/d/${NEW_TOKEN}$`));
    expect(creates).toHaveLength(1);
    await expect(
      page.getByRole("button", { name: "Switch endpoint", exact: true }),
    ).toContainText(NEW_TOKEN);
    await expect(page.getByText("No requests yet")).toBeVisible();
  });
});

test.describe("owner layout frames", () => {
  test("a 5xx loading the endpoint shows a retry message, and Retry recovers", async ({
    page,
  }) => {
    await installMockBackend(page, { authed: true, endpointFailures: 1 });
    await page.goto(`/d/${TOKEN}`);
    await expect(
      page.getByRole("heading", { name: "Couldn't load this endpoint" }),
    ).toBeVisible();
    await page.getByRole("button", { name: "Retry", exact: true }).click();
    await expect(page.getByRole("option").first()).toBeVisible();
    await expect(
      page.getByRole("heading", { name: "Couldn't load this endpoint" }),
    ).toHaveCount(0);
  });

  test("a persistent 5xx keeps the retry message instead of a stuck skeleton", async ({
    page,
  }) => {
    await installMockBackend(page, { authed: true, endpointStatus: 500 });
    await page.goto(`/d/${TOKEN}`);
    await expect(
      page.getByRole("heading", { name: "Couldn't load this endpoint" }),
    ).toBeVisible();
    await expect(page.locator('[aria-busy="true"]')).toHaveCount(0);
  });

  test("an unknown route renders NotFound with a way back", async ({ page }) => {
    await installMockBackend(page, { authed: false });
    await page.goto("/definitely/not/here");
    await expect(
      page.getByRole("heading", { level: 1, name: "Page not found" }),
    ).toBeVisible();
    await expect(
      page.getByRole("link", { name: "Back to start", exact: true }),
    ).toHaveAttribute("href", "/");
  });
});
