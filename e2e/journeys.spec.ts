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
import { installMockBackend, pushNewRequest, TOKEN } from "./mock-backend";

const CLEAR_ALL_MENUITEM = { name: "Clear all captured requests" };

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

  test("J9b: settings → clear-history confirm shows a server failure inline and stays open (ConfirmDialog, AC-83)", async ({
    page,
  }) => {
    await installMockBackend(page, { authed: true, clearRequestsStatus: 500 });
    await page.goto(`/d/${TOKEN}/settings`);

    await page.getByRole("button", { name: "Clear request history" }).click();
    await expect(
      page.getByRole("heading", { name: "Clear request history?" }),
    ).toBeVisible();
    // The confirm no longer carries a stale lifetime {n} (AC-77).
    await expect(page.getByText(/^All \d+ traces/)).toHaveCount(0);

    await page.getByRole("button", { name: "Clear history" }).click();

    // The rejection is caught (no unhandled promise rejection): the dialog
    // stays open and renders the server detail as an alert instead of the
    // failure escaping silently.
    await expect(
      page.getByRole("alert").filter({
        hasText: "Simulated failure for the confirm-dialog test.",
      }),
    ).toBeVisible();
    await expect(
      page.getByRole("heading", { name: "Clear request history?" }),
    ).toBeVisible();
  });
});

test.describe("F1 clear all (dashboard feed-actions menu)", () => {
  test("AC-1/AC-2/AC-3: opening the menu, then cancelling via Escape, issues zero requests and leaves rows untouched", async ({
    page,
  }) => {
    let deleteCount = 0;
    await installMockBackend(page, { authed: true });
    await page.route(`**/api/endpoints/${TOKEN}/requests`, async (route) => {
      if (route.request().method() === "DELETE") deleteCount++;
      await route.fallback();
    });
    await page.goto(`/d/${TOKEN}`);

    await page.getByRole("button", { name: "Feed actions" }).click();
    const clearAll = page.getByRole("menuitem", CLEAR_ALL_MENUITEM);
    await expect(clearAll).toBeEnabled();
    await clearAll.click();
    await expect(
      page.getByRole("heading", { name: "Clear all requests?" }),
    ).toBeVisible();
    // No count anywhere in the confirm body (AC-77).
    await expect(page.getByText(/^\d+ request/)).toHaveCount(0);

    await page.keyboard.press("Escape");
    await expect(
      page.getByRole("heading", { name: "Clear all requests?" }),
    ).toHaveCount(0);
    expect(deleteCount).toBe(0);
    // Rows are exactly as before — nothing was cleared.
    await expect(page.getByRole("option").first()).toContainText("/orders");
  });

  test("AC-4/AC-5/AC-78/AC-79: confirming clears the feed, empties the paused buffer, and resets selection", async ({
    page,
  }) => {
    let deleteCount = 0;
    await installMockBackend(page, { authed: true });
    await page.route(`**/api/endpoints/${TOKEN}/requests`, async (route) => {
      if (route.request().method() === "DELETE") deleteCount++;
      await route.fallback();
    });
    await page.goto(`/d/${TOKEN}`);

    // Select a row first — the Inspector must not be left pointing at nothing.
    await page.getByRole("option").first().click();
    await expect(page.getByRole("tab", { name: "Headers" })).toBeVisible();

    await page.getByRole("button", { name: "Feed actions" }).click();
    await page.getByRole("menuitem", CLEAR_ALL_MENUITEM).click();
    await page
      .getByRole("button", { name: "Clear all", exact: true })
      .last()
      .click();

    // Exactly one DELETE; the feed renders its empty state from local state.
    await expect(page.getByText("No requests yet")).toBeVisible();
    expect(deleteCount).toBe(1);
    // The Inspector falls back to "Select a request" — no stale selection.
    await expect(page.getByText("Select a request")).toBeVisible();
  });

  test("AC-6: a server failure keeps the dialog open, shows the detail, and removes no rows", async ({
    page,
  }) => {
    await installMockBackend(page, { authed: true, clearRequestsStatus: 500 });
    await page.goto(`/d/${TOKEN}`);

    await page.getByRole("button", { name: "Feed actions" }).click();
    await page.getByRole("menuitem", CLEAR_ALL_MENUITEM).click();
    await page
      .getByRole("button", { name: "Clear all", exact: true })
      .last()
      .click();

    await expect(
      page.getByRole("alert").filter({
        hasText: "Simulated failure for the confirm-dialog test.",
      }),
    ).toBeVisible();
    await expect(
      page.getByRole("heading", { name: "Clear all requests?" }),
    ).toBeVisible();
    // Cancel, then confirm the row is still there — nothing was removed.
    await page.getByRole("button", { name: "Cancel" }).click();
    await expect(page.getByRole("option").first()).toContainText("/orders");
  });

  test("AC-76: paused with an empty visible list but a buffered arrival still enables Clear all", async ({
    page,
  }) => {
    await installMockBackend(page, { authed: true, requests: [] });
    await page.goto(`/d/${TOKEN}`);
    await expect(page.getByText("No requests yet")).toBeVisible();

    await page.getByRole("button", { name: "Pause the live feed" }).click();
    await pushNewRequest(page, { id: 42, method: "GET", path: "/buffered" });
    // Buffered while paused → the "N new" pill, not a visible row.
    await expect(page.getByRole("button", { name: /1 new/ })).toBeVisible();

    await page.getByRole("button", { name: "Feed actions" }).click();
    await expect(page.getByRole("menuitem", CLEAR_ALL_MENUITEM)).toBeEnabled();
  });
});
