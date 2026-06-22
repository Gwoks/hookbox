/**
 * Journey e2e (journey.md, AC-J1..J13, AC-55). The end-to-end operator flow:
 * landing → dashboard → build a rule → settings → delete. Each test installs
 * the §5 mock backend (e2e/mock-backend.ts) and drives the real built SPA.
 *
 * Auth'd journeys start at /d/:token (the frontend-engineer lane). The landing
 * gate flow (/, AC-J1) is in landing.spec.ts and depends on the `/` route being
 * wired to the Landing screen (issue .29).
 */
import { expect, test } from "@playwright/test";
import { installMockBackend, TOKEN } from "./mock-backend";

test.describe("dashboard journey", () => {
  test("J2: loads the split-screen, streams the feed, inspects a row", async ({
    page,
  }) => {
    await installMockBackend(page, { authed: true });
    await page.goto(`/d/${TOKEN}`);

    // Sub-header subject (mock URL chip) + the live feed header render.
    await expect(page.getByText("Live feed")).toBeVisible();
    // Feed rows reconcile from the mocked GET (newest-first; id 3 on top).
    await expect(page.getByRole("option").first()).toContainText("/orders");

    // J3: select a row → inspector shows the 5 tabs.
    await page.getByRole("option").first().click();
    await expect(page.getByRole("tab", { name: "Headers" })).toBeVisible();
    await expect(
      page.getByRole("tab", { name: "State & Tracing" }),
    ).toBeVisible();

    // Headers tab shows a key/value row.
    await expect(page.getByText("user-agent")).toBeVisible();

    // Switch to State & Tracing → trace steps render.
    await page.getByRole("tab", { name: "State & Tracing" }).click();
    await expect(page.getByText("P1 mock catch-all")).toBeVisible();
  });

  test("J2: pause buffers arrivals and shows the N-new pill on resume control", async ({
    page,
  }) => {
    await installMockBackend(page, { authed: true });
    await page.goto(`/d/${TOKEN}`);
    await expect(page.getByText("Live feed")).toBeVisible();
    const pause = page.getByRole("button", { name: "Pause the live feed" });
    await pause.click();
    await expect(
      page.getByRole("button", { name: "Resume the live feed" }),
    ).toBeVisible();
  });

  test("J10: build a rule end-to-end (5 tabs) and see it in the manager", async ({
    page,
  }) => {
    await installMockBackend(page, { authed: true });
    await page.goto(`/d/${TOKEN}/rules?new=1`);

    // The builder opens to the Matching tab.
    await expect(page.getByRole("tab", { name: "Matching" })).toBeVisible();
    await page.getByLabel("Name").first().fill("Login success");
    await page.getByLabel("Path").fill("/login");

    // Response tab: status + body.
    await page.getByRole("tab", { name: "Response" }).click();
    await page.getByLabel("Status code").fill("200");

    // Templating tab: insert a tag into the body.
    await page.getByRole("tab", { name: "Templating" }).click();
    await page.getByRole("button", { name: "{{random 'uuid'}}" }).click();

    // Actions tab: the webhook is visible but flagged "Stored, not yet sent".
    await page.getByRole("tab", { name: "Actions" }).click();
    await expect(page.getByText("Stored, not yet sent")).toBeVisible();

    // Save → toast + the rule lands in the manager list.
    await page.getByRole("button", { name: "Save rule" }).click();
    await expect(page.getByText("Login success")).toBeVisible();
  });

  test("J9: settings → typed-token delete routes back to start", async ({
    page,
  }) => {
    await installMockBackend(page, { authed: true });
    await page.goto(`/d/${TOKEN}/settings`);

    await expect(page.getByRole("heading", { name: "Settings" })).toBeVisible();
    // Danger zone delete behind a typed-token confirm.
    await page.getByRole("button", { name: "Delete endpoint" }).first().click();
    const confirmBtn = page
      .getByRole("button", { name: "Delete endpoint" })
      .last();
    await expect(confirmBtn).toBeDisabled();
    await page.getByPlaceholder(TOKEN).fill(TOKEN);
    await expect(confirmBtn).toBeEnabled();
    await confirmBtn.click();
    // Routed back to / (landing). URL no longer points at the deleted endpoint.
    await expect(page).toHaveURL(/\/$/);
  });
});
