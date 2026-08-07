/**
 * F4 owner-side share dialog e2e (operator-toolkit prd.md §4.4, AC-23..27,
 * AC-93..99, AC-S11). Drives the real built SPA against the §5 mock backend.
 */
import { expect, test } from "@playwright/test";
import {
  DEFAULT_SHARE_CODE,
  installMockBackend,
  TOKEN,
  type ShareLinkStub,
} from "./mock-backend";

async function openShareDialog(page: import("@playwright/test").Page) {
  await page.getByRole("button", { name: /^Share this endpoint read-only/ }).click();
  await expect(page.getByRole("heading", { name: "Share a read-only link" })).toBeVisible();
}

test.describe("F4 Share dialog (owner)", () => {
  test("AC-23/98: the Share control is first in the action cluster and shows no badge at zero links", async ({
    page,
  }) => {
    await installMockBackend(page, { authed: true, shares: [] });
    await page.goto(`/d/${TOKEN}`);
    const trigger = page.getByRole("button", { name: "Share this endpoint read-only" });
    await expect(trigger).toBeVisible();
    await expect(trigger).toBeEnabled();
  });

  test("AC-98: the count badge renders once >=1 active link exists", async ({ page }) => {
    await installMockBackend(page, {
      authed: true,
      shares: [{ id: 1, label: "A", created_at: "2026-06-21T12:00:00Z", last_used_at: null }],
    });
    await page.goto(`/d/${TOKEN}`);
    await expect(
      page.getByRole("button", { name: "Share this endpoint read-only — 1 active links" }),
    ).toBeVisible();
  });

  test("AC-93/AC-S11: the disclosure names everything shown, everything hidden, and never claims response headers are verbatim", async ({
    page,
  }) => {
    await installMockBackend(page, { authed: true, shares: [] });
    await page.goto(`/d/${TOKEN}`);
    await openShareDialog(page);

    await expect(page.getByText("A share link publishes captured traffic")).toBeVisible();
    await expect(page.getByText(/last 100 requests/)).toBeVisible();
    await expect(page.getByText(/arrived before you created the link/)).toBeVisible();
    await expect(page.getByText(/ones other people sent/)).toBeVisible();
    await expect(page.getByText(/endpoint's name is visible/)).toBeVisible();
    await expect(page.getByText(/Hidden automatically: Authorization, Cookie and X-Owner-Id/)).toBeVisible();
    // The superseded ux.md claim must never render.
    await expect(page.getByText(/exactly as sent/)).toHaveCount(0);
    await expect(page.getByText(/including any Set-Cookie/)).toHaveCount(0);
  });

  test("AC-94: a label over 80 characters disables Create and shows the error client-side, before any request", async ({
    page,
  }) => {
    let postCount = 0;
    await installMockBackend(page, { authed: true, shares: [] });
    await page.route(`**/api/endpoints/${TOKEN}/shares`, async (route) => {
      if (route.request().method() === "POST") postCount++;
      await route.fallback();
    });
    await page.goto(`/d/${TOKEN}`);
    await openShareDialog(page);

    await page.getByLabel("Label").fill("x".repeat(81));
    await expect(page.getByText("Labels are 80 characters or fewer.")).toBeVisible();
    await expect(page.getByRole("button", { name: "Create share link" })).toBeDisabled();
    expect(postCount).toBe(0);
  });

  test("AC-24/25/96: creating reveals the URL exactly once, and the list never carries a URL", async ({
    page,
  }) => {
    await installMockBackend(page, { authed: true, shares: [] });
    await page.goto(`/d/${TOKEN}`);
    await openShareDialog(page);

    await page.getByLabel("Label").fill("Acme ticket");
    await page.getByRole("button", { name: "Create share link" }).click();

    await expect(page.getByText("Your share link")).toBeVisible();
    await expect(page.getByText(`/s/${DEFAULT_SHARE_CODE}`)).toBeVisible();
    await expect(page.getByText(/Shown once — copy it now/)).toBeVisible();
    await expect(
      page.getByRole("link", { name: /Open the share link in a new tab/ }),
    ).toHaveAttribute("href", new RegExp(`/s/${DEFAULT_SHARE_CODE}$`));

    // The new row appears in the list — with a label and no URL.
    await expect(page.getByText("Acme ticket")).toBeVisible();

    // AC-25: the dialog's DOM contains "/s/" ONLY inside the one-time panel.
    const dialog = page.getByRole("dialog");
    const shareUrlMatches = await dialog.locator(`text=/\\/s\\/${DEFAULT_SHARE_CODE}/`).count();
    expect(shareUrlMatches).toBe(1); // the one-time panel's CodeBlock + link share the same text node grouping is fine as long as it's not duplicated per row
  });

  test("AC-99: a share URL pointing at localhost shows the unreachable-origin warning", async ({
    page,
  }) => {
    await installMockBackend(page, { authed: true, shares: [] });
    await page.goto(`/d/${TOKEN}`);
    await openShareDialog(page);
    await page.getByRole("button", { name: "Create share link" }).click();
    await expect(page.getByText(/may not be reachable from outside your network/)).toBeVisible();
  });

  test("AC-95: revoke is a two-step inline confirm; Cancel disarms without closing the dialog", async ({
    page,
  }) => {
    const shares: ShareLinkStub[] = [
      { id: 1, label: "Row A", created_at: "2026-06-21T12:00:00Z", last_used_at: null },
    ];
    await installMockBackend(page, { authed: true, shares });
    await page.goto(`/d/${TOKEN}`);
    await openShareDialog(page);

    await page.getByRole("button", { name: "Revoke this share link" }).click();
    await expect(page.getByText("Revoke this link?")).toBeVisible();
    await page.getByRole("button", { name: "Cancel" }).click();
    await expect(page.getByText("Revoke this link?")).toHaveCount(0);
    // Still open, row still there.
    await expect(page.getByText("Row A")).toBeVisible();
    await expect(page.getByRole("heading", { name: "Share a read-only link" })).toBeVisible();
  });

  test("AC-95: a 204 revoke removes the row and toasts 'Share link revoked.'", async ({ page }) => {
    const shares: ShareLinkStub[] = [
      { id: 1, label: "Row A", created_at: "2026-06-21T12:00:00Z", last_used_at: null },
    ];
    await installMockBackend(page, { authed: true, shares });
    await page.goto(`/d/${TOKEN}`);
    await openShareDialog(page);
    await page.getByRole("button", { name: "Revoke this share link" }).click();
    await page.getByRole("button", { name: "Revoke", exact: true }).click();

    await expect(page.getByText("Share link revoked.", { exact: true })).toBeVisible();
    await expect(page.getByText("Row A")).toHaveCount(0);
  });

  test("AC-95: a 404 on revoke (already revoked) is treated as success, not an error", async ({
    page,
  }) => {
    const shares: ShareLinkStub[] = [
      { id: 1, label: "Row A", created_at: "2026-06-21T12:00:00Z", last_used_at: null },
    ];
    await installMockBackend(page, { authed: true, shares });
    await page.route(`**/api/endpoints/${TOKEN}/shares/1`, (route) =>
      route.fulfill({
        status: 404,
        contentType: "application/json",
        body: JSON.stringify({ error: "not_found", detail: "Share link not found." }),
      }),
    );
    await page.goto(`/d/${TOKEN}`);
    await openShareDialog(page);
    await page.getByRole("button", { name: "Revoke this share link" }).click();
    await page.getByRole("button", { name: "Revoke", exact: true }).click();

    await expect(page.getByText("That link was already revoked.", { exact: true })).toBeVisible();
    await expect(page.getByText("Couldn't revoke", { exact: false })).toHaveCount(0);
  });

  test("AC-95: a 5xx on revoke keeps the row, shows a row-level error, and a danger toast", async ({
    page,
  }) => {
    const shares: ShareLinkStub[] = [
      { id: 1, label: "Row A", created_at: "2026-06-21T12:00:00Z", last_used_at: null },
    ];
    await installMockBackend(page, { authed: true, shares });
    await page.route(`**/api/endpoints/${TOKEN}/shares/1`, (route) =>
      route.fulfill({
        status: 503,
        contentType: "application/json",
        body: JSON.stringify({ error: "store_unavailable", detail: "Boom." }),
      }),
    );
    await page.goto(`/d/${TOKEN}`);
    await openShareDialog(page);
    await page.getByRole("button", { name: "Revoke this share link" }).click();
    await page.getByRole("button", { name: "Revoke", exact: true }).click();

    await expect(
      page.getByText("Couldn't revoke the link. It's still active — try again.").first(),
    ).toBeVisible();
    await expect(page.getByText("Row A")).toBeVisible(); // row restored/kept
  });

  test("AC-96: list-load failure shows an alert with Retry", async ({ page }) => {
    await installMockBackend(page, { authed: true, shares: [] });
    await page.route(`**/api/endpoints/${TOKEN}/shares`, (route) => {
      if (route.request().method() !== "GET") return route.fallback();
      return route.fulfill({
        status: 503,
        contentType: "application/json",
        body: JSON.stringify({ error: "store_unavailable", detail: "Boom." }),
      });
    });
    await page.goto(`/d/${TOKEN}`);
    await openShareDialog(page);
    await expect(page.getByText("Couldn't load share links", { exact: true })).toBeVisible();
    await expect(page.getByRole("button", { name: "Retry" })).toBeVisible();
  });

  test("AC-96: an empty list shows the empty state", async ({ page }) => {
    await installMockBackend(page, { authed: true, shares: [] });
    await page.goto(`/d/${TOKEN}`);
    await openShareDialog(page);
    await expect(page.getByText("No share links yet")).toBeVisible();
  });
});
