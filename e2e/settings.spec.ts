/**
 * Settings screen e2e (FE-P5): the section index anchors, the sticky save
 * bar's unsaved-changes / Discard cycle, and proxy URL validation.
 */
import { expect, test } from "@playwright/test";
import { installMockBackend, TOKEN } from "./mock-backend";

test.describe("settings", () => {
  test("the section index jumps to each section by anchor", async ({ page }) => {
    await page.setViewportSize({ width: 1440, height: 900 });
    await installMockBackend(page, { authed: true });
    await page.goto(`/d/${TOKEN}/settings`);
    const index = page.getByRole("navigation", { name: "Settings sections" });
    await index.getByRole("link", { name: "Simulated conditions", exact: true }).click();
    await expect(page).toHaveURL(new RegExp(`/d/${TOKEN}/settings#conditions$`));
    await expect(page.getByRole("region", { name: "Simulated conditions" })).toBeInViewport();

    await index.getByRole("link", { name: "Danger zone", exact: true }).click();
    await expect(page.getByRole("region", { name: "Danger zone" })).toBeInViewport();
  });

  test("the save bar tracks unsaved changes and Discard restores the saved values", async ({
    page,
  }) => {
    await installMockBackend(page, { authed: true });
    await page.goto(`/d/${TOKEN}/settings`);
    const name = page.getByLabel("Endpoint name", { exact: true });
    await expect(page.getByText("All changes saved", { exact: true })).toBeVisible();
    await expect(page.getByRole("button", { name: "Discard", exact: true })).toBeDisabled();

    await name.fill("Renamed API");
    await expect(page.getByText("Unsaved changes", { exact: true })).toBeVisible();
    await page.getByRole("button", { name: "Discard", exact: true }).click();
    await expect(name).toHaveValue("Checkout API");
    await expect(page.getByText("All changes saved", { exact: true })).toBeVisible();

    await name.fill("Renamed API");
    await page.getByRole("button", { name: "Save", exact: true }).click();
    await expect(page.getByText("Settings saved.", { exact: true })).toBeVisible();
    await expect(page.getByText("All changes saved", { exact: true })).toBeVisible();
    // The shared endpoint context renamed the switcher too.
    await expect(
      page.getByRole("button", { name: "Switch endpoint", exact: true }),
    ).toContainText("Renamed API");
  });

  test("an invalid proxy target is flagged on blur and blocks Save", async ({ page }) => {
    let patches = 0;
    await installMockBackend(page, { authed: true });
    page.on("request", (request) => {
      if (request.method() === "PATCH") patches += 1;
    });
    await page.goto(`/d/${TOKEN}/settings`);
    const target = page.getByLabel("Target URL", { exact: true });
    await target.fill("ftp://files.example.com");
    await target.blur();
    await expect(page.getByText("Enter an http or https URL with a host.")).toBeVisible();
    await expect(target).toHaveAttribute("aria-invalid", "true");
    await page.getByRole("button", { name: "Save", exact: true }).click();
    await expect(page.getByText("Settings saved.", { exact: true })).toHaveCount(0);
    expect(patches).toBe(0);
  });
});
