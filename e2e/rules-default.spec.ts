/**
 * F6 "Add default rule" e2e (operator-toolkit prd.md §4.6, AC-57..61,
 * AC-122..125). Drives the real built SPA against the §5 mock backend.
 */
import { expect, test } from "@playwright/test";
import {
  installMockBackend,
  makeRule,
  TOKEN,
} from "./mock-backend";

const ADD_DEFAULT = { name: "Add a catch-all rule that answers any unmatched request" };

test.describe("F6 Add default rule (Rules Manager)", () => {
  test("AC-58/AC-60: zero active fallbacks → straight to the POST, no confirm, exact §5.5.7 body", async ({
    page,
  }) => {
    let createBody: unknown = null;
    let postCount = 0;
    await installMockBackend(page, {
      authed: true,
      rules: [],
      endpoint: {
        auto_crud: false,
        tunnel_active: false,
        target_url: null,
        default_mode: "mock_404",
      },
    });
    await page.route(
      `**/api/endpoints/${TOKEN}/rules`,
      async (route) => {
        if (route.request().method() === "POST") {
          postCount++;
          createBody = route.request().postDataJSON();
        }
        await route.fallback();
      },
    );
    await page.goto(`/d/${TOKEN}/rules`);

    await page.getByRole("button", ADD_DEFAULT).first().click();
    // No shadow confirm — the common one-click path (AC-122c).
    await expect(
      page.getByRole("heading", { name: "This will take over unmatched requests" }),
    ).toHaveCount(0);

    await expect(
      page.getByText("Default catch-all rule added.", { exact: true }),
    ).toBeVisible();
    expect(postCount).toBe(1);
    expect(createBody).toEqual({
      name: "Catch-all (default)",
      priority: 1000,
      enabled: true,
      match: {
        method: "ANY",
        path: "/*",
        headers: {},
        query: {},
        body_conditions: [],
        state_requirements: [],
      },
      response: {
        status_code: 200,
        headers: {},
        content_type: "application/json",
        body_template:
          '{\n  "ok": true,\n  "hookbox": "default catch-all",\n  "message": "Edit this rule in HookBox to return your own response."\n}',
      },
      state_writes: [],
      latency_ms: null,
      rate_limit_per_min: null,
      chaos_mode: null,
      webhook_action: null,
    });
    // The new rule lands in the list with ANY / *.
    await expect(page.getByText("Catch-all (default)")).toBeVisible();
  });

  test("AC-122(a)/(b): exactly one active fallback (auto_crud) → exactly one shadow bullet", async ({
    page,
  }) => {
    // The mock's default endpoint has auto_crud: true and every other
    // fallback off — exactly one bullet should render.
    await installMockBackend(page, { authed: true, rules: [] });
    await page.goto(`/d/${TOKEN}/rules`);

    await page.getByRole("button", ADD_DEFAULT).first().click();
    await expect(
      page.getByRole("heading", { name: "This will take over unmatched requests" }),
    ).toBeVisible();
    await expect(page.getByText("Auto-CRUD stops serving this endpoint.")).toBeVisible();
    // The other three fallbacks are inactive — their bullets must NOT render.
    await expect(page.getByText("Your tunnel stops receiving requests.")).toHaveCount(0);
    await expect(
      page.getByText("Requests stop being proxied to your target URL."),
    ).toHaveCount(0);
    await expect(page.getByText("The Echo default response stops being used.")).toHaveCount(0);
  });

  test("AC-122(d): cancelling the shadow confirm issues zero requests", async ({
    page,
  }) => {
    let postCount = 0;
    await installMockBackend(page, { authed: true, rules: [] });
    await page.route(`**/api/endpoints/${TOKEN}/rules`, async (route) => {
      if (route.request().method() === "POST") postCount++;
      await route.fallback();
    });
    await page.goto(`/d/${TOKEN}/rules`);

    await page.getByRole("button", ADD_DEFAULT).first().click();
    await expect(
      page.getByRole("heading", { name: "This will take over unmatched requests" }),
    ).toBeVisible();
    await page.getByRole("button", { name: "Cancel" }).click();
    await expect(
      page.getByRole("heading", { name: "This will take over unmatched requests" }),
    ).toHaveCount(0);
    expect(postCount).toBe(0);
  });

  test("AC-61/AC-123(a): an existing catch-all (even disabled) blocks a second, with the right reason", async ({
    page,
  }) => {
    await installMockBackend(page, {
      authed: true,
      rules: [
        makeRule(1, {
          name: "Catch-all (default)",
          enabled: false,
          match: { method: "ANY", path: "/*" },
        }),
      ],
    });
    await page.goto(`/d/${TOKEN}/rules`);

    const addButtons = page.getByRole("button", ADD_DEFAULT);
    await expect(addButtons.first()).toBeDisabled();
    // Reachable by keyboard: the wrapping span is focusable and carries the
    // reason both as a native title (no-JS fallback) and a Radix tooltip.
    const wrapper = page.locator('span[title="This endpoint already has a catch-all rule, but it\'s switched off. Turn it back on instead of adding another."]').first();
    await expect(wrapper).toHaveCount(1);
  });

  test("AC-123(c): a stale list (another tab already added one) refreshes instead of duplicating", async ({
    page,
  }) => {
    let getCount = 0;
    let postCount = 0;
    await installMockBackend(page, {
      authed: true,
      rules: [],
      endpoint: {
        auto_crud: false,
        tunnel_active: false,
        target_url: null,
        default_mode: "mock_404",
      },
    });
    await page.route(`**/api/endpoints/${TOKEN}/rules`, async (route) => {
      const req = route.request();
      if (req.method() === "GET") {
        getCount++;
        if (getCount === 1) {
          // Initial load: no catch-all yet.
          return route.fulfill({
            status: 200,
            contentType: "application/json",
            body: "[]",
          });
        }
        // The re-check right before creating: another tab beat us to it.
        return route.fulfill({
          status: 200,
          contentType: "application/json",
          body: JSON.stringify([
            makeRule(1, {
              name: "Catch-all (default)",
              match: { method: "ANY", path: "/*" },
            }),
          ]),
        });
      }
      if (req.method() === "POST") postCount++;
      await route.fallback();
    });
    await page.goto(`/d/${TOKEN}/rules`);

    await page.getByRole("button", ADD_DEFAULT).first().click();
    await expect(
      page.getByText(
        "This endpoint already has a catch-all rule. The list has been refreshed.",
        { exact: true },
      ),
    ).toBeVisible();
    expect(postCount).toBe(0);
    // The list now reflects the freshly-fetched catch-all.
    await expect(page.getByText("Catch-all (default)")).toBeVisible();
  });

  test("AC-123(b): the control disables itself while the POST is in flight", async ({
    page,
  }) => {
    await installMockBackend(page, {
      authed: true,
      rules: [],
      endpoint: {
        auto_crud: false,
        tunnel_active: false,
        target_url: null,
        default_mode: "mock_404",
      },
    });
    await page.route(`**/api/endpoints/${TOKEN}/rules`, async (route) => {
      if (route.request().method() === "POST") {
        await new Promise((r) => setTimeout(r, 400));
      }
      await route.fallback();
    });
    await page.goto(`/d/${TOKEN}/rules`);

    const button = page.getByRole("button", ADD_DEFAULT).first();
    await button.click();
    await expect(button).toBeDisabled();
    await expect(
      page.getByText("Default catch-all rule added.", { exact: true }),
    ).toBeVisible();
  });
});
