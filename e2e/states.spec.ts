/**
 * Per-screen state-matrix e2e (journey.md state matrices, AC-J2/J3/J4). Drives
 * the dashboard shell, feed, and inspector through their documented states with
 * the §5 mock backend. Each case forces a specific backend response.
 */
import { expect, test } from "@playwright/test";
import { installMockBackend, makeRequest, TOKEN } from "./mock-backend";

test.describe("dashboard shell states (AC-J2)", () => {
  test("not-signed-in → bounces to the landing gate", async ({ page }) => {
    await installMockBackend(page, { authed: false });
    await page.goto(`/d/${TOKEN}`);
    await expect(page).toHaveURL(/\/$/);
  });

  test("404 → distinct 'Endpoint not found' card", async ({ page }) => {
    await installMockBackend(page, { authed: true, endpointStatus: 404 });
    await page.goto(`/d/${TOKEN}`);
    await expect(
      page.getByRole("heading", { name: "Endpoint not found" }),
    ).toBeVisible();
  });

  test("410 → distinct 'Endpoint deleted' card (not the 404 copy)", async ({
    page,
  }) => {
    await installMockBackend(page, { authed: true, endpointStatus: 410 });
    await page.goto(`/d/${TOKEN}`);
    await expect(
      page.getByRole("heading", { name: "Endpoint deleted" }),
    ).toBeVisible();
    await expect(page.getByText("Endpoint not found")).toHaveCount(0);
  });
});

test.describe("feed states (AC-J2)", () => {
  test("empty → mock-URL block + a static curl sample (never executed)", async ({
    page,
  }) => {
    await installMockBackend(page, { authed: true, requests: [] });
    await page.goto(`/d/${TOKEN}`);
    await expect(page.getByText("No requests yet")).toBeVisible();
    await expect(page.getByText(/curl https:\/\//)).toBeVisible();
  });

  test("streaming → rows newest-first, capped count label", async ({
    page,
  }) => {
    await installMockBackend(page, { authed: true });
    await page.goto(`/d/${TOKEN}`);
    const rows = page.getByRole("option");
    await expect(rows.first()).toContainText("/orders");
    await expect(page.getByText(/Showing \d+ of last 100/)).toBeVisible();
  });
});

test.describe("inspector states (AC-J3/J4)", () => {
  test("empty → 'Select a request'", async ({ page }) => {
    await installMockBackend(page, { authed: true });
    await page.goto(`/d/${TOKEN}`);
    await expect(page.getByText("Select a request")).toBeVisible();
  });

  test("pending → a streamed row whose detail 404s shows 'Detail on its way' + Retry, not a hard 404", async ({
    page,
  }) => {
    // id 999 is the PENDING sentinel in the mock (#13 → 404).
    await installMockBackend(page, {
      authed: true,
      requests: [
        makeRequest({
          id: 999,
          method: "POST",
          path: "/just-now",
          status_code: 201,
          served_by: "crud",
        }),
      ],
    });
    await page.goto(`/d/${TOKEN}`);
    await page.getByRole("option").first().click();
    await expect(page.getByText("Detail on its way")).toBeVisible();
    await expect(page.getByRole("button", { name: "Retry" })).toBeVisible();
    // It is NOT the hard not-found shell card.
    await expect(page.getByText("Endpoint not found")).toHaveCount(0);
  });

  test("ready → 5 tabs and response-served content", async ({ page }) => {
    await installMockBackend(page, { authed: true });
    await page.goto(`/d/${TOKEN}`);
    await page.getByRole("option").first().click();
    await page.getByRole("tab", { name: "Response Served" }).click();
    await expect(page.getByText("Response headers")).toBeVisible();
  });
});

test.describe("rules manager states (AC-J10)", () => {
  test("empty → resolution-order honesty note", async ({ page }) => {
    await installMockBackend(page, { authed: true, rules: [] });
    await page.goto(`/d/${TOKEN}/rules`);
    await expect(page.getByText("No rules yet")).toBeVisible();
    // The honesty note explains the fall-through resolution order.
    await expect(
      page.getByText(/unmatched requests use Auto-CRUD|fall through/),
    ).toBeVisible();
  });
});
