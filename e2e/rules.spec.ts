/**
 * Rules list e2e (FE-P5): the per-row enable switch is named after its rule
 * ("Enabled: {name}" — it used to read as a toast string), and delete goes
 * through the shared ConfirmDialog.
 */
import { expect, test } from "@playwright/test";
import { installMockBackend, makeRule, TOKEN } from "./mock-backend";

const RULES = [
  makeRule(1, { name: "Login success", priority: 10, match: { method: "POST", path: "/login" } }),
  makeRule(2, { name: null, priority: 20, enabled: false }),
];

test.describe("rules list", () => {
  test("each enable switch is named after its rule and toggles it", async ({ page }) => {
    await installMockBackend(page, { authed: true, rules: RULES });
    await page.goto(`/d/${TOKEN}/rules`);
    await expect(page.getByRole("heading", { level: 1, name: "Rules" })).toBeVisible();

    const login = page.getByRole("switch", { name: "Enabled: Login success", exact: true });
    const untitled = page.getByRole("switch", { name: "Enabled: Untitled rule", exact: true });
    await expect(login).toHaveAttribute("aria-checked", "true");
    await expect(untitled).toHaveAttribute("aria-checked", "false");

    await login.click();
    await expect(login).toHaveAttribute("aria-checked", "false");
    await expect(page.getByText("Rule disabled.", { exact: true })).toBeVisible();
  });

  test("delete asks for confirmation in a dialog, then removes the row", async ({ page }) => {
    await installMockBackend(page, { authed: true, rules: RULES });
    await page.goto(`/d/${TOKEN}/rules`);
    await page.getByRole("button", { name: "Rule actions" }).first().click();
    await page.getByRole("menuitem", { name: "Delete", exact: true }).click();

    const dialog = page.getByRole("dialog", { name: "Delete this rule?" });
    await expect(dialog).toContainText('"Login success" will stop matching requests.');
    await dialog.getByRole("button", { name: "Delete rule", exact: true }).click();

    await expect(dialog).toHaveCount(0);
    await expect(page.getByText("Rule deleted.", { exact: true })).toBeVisible();
    await expect(page.getByRole("switch", { name: "Enabled: Login success" })).toHaveCount(0);
  });
});
