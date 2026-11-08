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

test.describe("rule builder", () => {
  test("each list editor is a group named by its legend, with labelled cells", async ({
    page,
  }) => {
    await installMockBackend(page, { authed: true, rules: [] });
    await page.goto(`/d/${TOKEN}/rules?new=1`);
    const dialog = page.getByRole("dialog", { name: "New rule" });

    for (const legend of [
      "Required headers",
      "Required query params",
      "Body conditions",
      "State requirements",
    ]) {
      await expect(dialog.getByRole("group", { name: legend, exact: true })).toBeVisible();
    }
    const body = dialog.getByRole("group", { name: "Body conditions", exact: true });
    await body.getByRole("button", { name: "Add condition", exact: true }).click();
    await expect(body.getByRole("textbox", { name: "Field 1", exact: true })).toBeVisible();
    const operator = body.getByRole("combobox", { name: "Operator 1", exact: true });
    await expect(operator).toHaveValue("eq");
    await operator.selectOption("exists");
    await expect(body.getByRole("textbox", { name: "Value 1", exact: true })).toBeDisabled();

    await dialog.getByRole("tab", { name: "Response", exact: true }).click();
    await expect(dialog.getByRole("group", { name: "Response headers", exact: true })).toBeVisible();
    await dialog.getByRole("tab", { name: "Actions", exact: true }).click();
    await expect(dialog.getByRole("group", { name: "State writes", exact: true })).toBeVisible();
    await expect(dialog.getByRole("group", { name: /^Webhook/ })).toContainText("Stored, not yet sent");
    await expect(dialog.getByLabel("Webhook URL", { exact: true })).toBeVisible();
  });

  test("a tab with an invalid field shows its count and Save jumps to it", async ({ page }) => {
    await installMockBackend(page, { authed: true, rules: [] });
    await page.goto(`/d/${TOKEN}/rules?new=1`);
    const dialog = page.getByRole("dialog", { name: "New rule" });
    await dialog.getByLabel("Path", { exact: true }).fill("login");
    await expect(dialog.getByRole("tab", { name: "Matching (1 to fix)", exact: true })).toBeVisible();
    await expect(dialog.getByText("1 fields need attention")).toBeVisible();

    await dialog.getByRole("tab", { name: "Throttling", exact: true }).click();
    await dialog.getByRole("button", { name: "Save rule", exact: true }).click();
    await expect(dialog.getByRole("tab", { name: /^Matching/ })).toHaveAttribute("aria-selected", "true");
    await expect(dialog.getByText("Enter a path starting with /.")).toBeVisible();
  });

  test("throttling overrides can return to Inherit (null)", async ({ page }) => {
    let patchBody: Record<string, unknown> | null = null;
    await installMockBackend(page, {
      authed: true,
      rules: [makeRule(1, { name: "Slow", latency_ms: 300 })],
    });
    page.on("request", (request) => {
      if (request.method() === "PATCH") patchBody = request.postDataJSON();
    });
    await page.goto(`/d/${TOKEN}/rules`);
    await page.getByRole("button", { name: "Slow", exact: true }).click();
    const dialog = page.getByRole("dialog", { name: "Edit rule" });
    await dialog.getByRole("tab", { name: "Throttling", exact: true }).click();

    const latency = dialog.getByRole("spinbutton", { name: "Latency", exact: true });
    await expect(latency).toHaveValue("300");
    await dialog.getByRole("switch", { name: "Inherit" }).first().click();
    await expect(latency).toBeDisabled();
    await dialog.getByRole("button", { name: "Save rule", exact: true }).click();
    await expect(dialog).toHaveCount(0);
    expect(patchBody).toMatchObject({ latency_ms: null, rate_limit_per_min: null });
  });
});
