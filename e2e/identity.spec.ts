/**
 * Anonymous-key identity e2e (hookbox-a73.3): the recovery-key card in
 * Settings (reveal / rotate), the CLI page's key reveal, sign-out, and the
 * `X-HookBox: 1` header every owner request must carry. The mock models the
 * HttpOnly session cookie as `store.authed` (see e2e/mock-backend.ts).
 */
import { expect, test, type Page } from "@playwright/test";
import {
  installMockBackend,
  ROTATED_KEY_PREFIX,
  SECRET,
  TOKEN,
} from "./mock-backend";

function recoveryCard(page: Page) {
  return page.getByRole("region", { name: "Recovery key" });
}

test.describe("Settings → recovery key", () => {
  test("reveals the key on demand and hides it again", async ({ page }) => {
    await installMockBackend(page, { authed: true });
    await page.goto(`/d/${TOKEN}/settings`);

    const card = recoveryCard(page);
    await expect(card).toHaveAttribute("id", "recovery-key");
    await expect(card).not.toContainText(SECRET);

    await card.getByRole("button", { name: "Reveal key" }).click();
    await expect(card.getByText(SECRET)).toBeVisible();
    await expect(card.getByRole("button", { name: "Copy key" })).toBeVisible();

    await card.getByRole("button", { name: "Hide key" }).click();
    await expect(card).not.toContainText(SECRET);
  });

  test("rotating asks first, then shows the new key and kills the old one", async ({ page }) => {
    const store = await installMockBackend(page, { authed: true });
    await page.goto(`/d/${TOKEN}/settings`);

    await recoveryCard(page).getByRole("button", { name: "Rotate key" }).click();
    const dialog = page.getByRole("dialog");
    await expect(dialog.getByRole("heading", { name: "Rotate your recovery key?" })).toBeVisible();
    await expect(dialog).toContainText("tunnel CLI disconnects");
    expect(store.rotations).toBe(0);

    await dialog.getByRole("button", { name: "Rotate key" }).click();

    const rotated = `${ROTATED_KEY_PREFIX}1`;
    await expect(recoveryCard(page).getByText(rotated)).toBeVisible();
    await expect(recoveryCard(page)).toContainText("New key ready");
    expect(store.key).toBe(rotated);
    expect(store.key).not.toBe(SECRET);
  });

  test("the account menu's Recovery key item opens the card", async ({ page }) => {
    await installMockBackend(page, { authed: true });
    await page.goto(`/d/${TOKEN}`);

    await page.getByRole("button", { name: "Account" }).click();
    await page.getByRole("menuitem", { name: "Recovery key" }).click();

    await expect(page).toHaveURL(new RegExp(`/d/${TOKEN}/settings#recovery-key$`));
    await expect(recoveryCard(page)).toBeInViewport();
  });
});

test.describe("CLI page", () => {
  test("Reveal puts the recovery key into the tunnel command", async ({ page }) => {
    await installMockBackend(page, { authed: true });
    await page.goto("/cli");

    const command = page.locator("code", { hasText: "tunnel --port" });
    await expect(command).toContainText(`--endpoint ${TOKEN}`);
    await expect(command).not.toContainText(SECRET);

    await page.getByRole("button", { name: "Reveal key" }).click();
    await expect(command).toContainText(`--key ${SECRET}`);

    await page.getByRole("button", { name: "Hide key" }).click();
    await expect(command).not.toContainText(SECRET);
  });

  test("signed out, it shows placeholders and never redirects", async ({ page }) => {
    await installMockBackend(page);
    await page.goto("/cli");

    const command = page.locator("code", { hasText: "tunnel --port" });
    await expect(command).toContainText("--endpoint <endpoint>");
    await expect(command).toContainText("--key <recovery-key>");
    await expect(page.getByRole("button", { name: "Reveal key" })).toHaveCount(0);
    await expect(page).toHaveURL(/\/cli$/);
  });
});

test.describe("sign out", () => {
  test("warns to save the key, offers a copy, then DELETEs the session and lands on /", async ({
    page,
    context,
  }) => {
    await context.grantPermissions(["clipboard-read", "clipboard-write"]);
    const sessionDeletes: string[] = [];
    page.on("request", (request) => {
      if (request.method() === "DELETE" && new URL(request.url()).pathname === "/api/session") {
        sessionDeletes.push(request.url());
      }
    });
    const store = await installMockBackend(page, { authed: true });
    await page.goto(`/d/${TOKEN}`);

    await page.getByRole("button", { name: "Account" }).click();
    await page.getByRole("menuitem", { name: "Sign out" }).click();

    const dialog = page.getByRole("dialog");
    await expect(dialog).toContainText("Save your recovery key first");
    await dialog.getByRole("button", { name: "Copy recovery key" }).click();
    await expect(dialog.getByText("Copied to clipboard")).toBeVisible();
    expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(SECRET);
    expect(sessionDeletes).toEqual([]);

    await dialog.getByRole("button", { name: "Sign out" }).click();

    await expect(page).toHaveURL(/\/$/);
    await expect(page.getByRole("button", { name: "Get my endpoint" })).toBeVisible();
    expect(sessionDeletes).toHaveLength(1);
    expect(store.authed).toBe(false);
  });
});

test("every owner request carries X-HookBox: 1", async ({ page }) => {
  const ownerRequests: { path: string; header: string | undefined }[] = [];
  page.on("request", (request) => {
    const { pathname } = new URL(request.url());
    if (pathname.startsWith("/api/") && !pathname.startsWith("/api/share/")) {
      ownerRequests.push({ path: pathname, header: request.headers()["x-hookbox"] });
    }
  });
  await installMockBackend(page, { authed: true });
  await page.goto(`/d/${TOKEN}`);
  await expect(page.getByRole("option").first()).toContainText("/orders");

  const paths = ownerRequests.map((r) => r.path);
  expect(paths).toContain("/api/session");
  expect(paths).toContain(`/api/endpoints/${TOKEN}`);
  expect(ownerRequests.filter((r) => r.header !== "1")).toEqual([]);
});
