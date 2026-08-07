/**
 * F3 "Export / import config as JSON" e2e (operator-toolkit prd.md §4.3,
 * §5.5.6, AC-11..22, AC-87..92, AC-S21/S22/S23). Drives the real built SPA
 * against the §5 mock backend.
 */
import type { Download, Page } from "@playwright/test";
import { expect, test } from "@playwright/test";
import { installMockBackend, makeRule, TOKEN } from "./mock-backend";

async function readDownloadText(download: Download): Promise<string> {
  const stream = await download.createReadStream();
  if (!stream) throw new Error("download produced no stream");
  const chunks: Buffer[] = [];
  for await (const chunk of stream) chunks.push(chunk as Buffer);
  return Buffer.concat(chunks).toString("utf-8");
}

async function importFile(page: Page, text: string) {
  await page.getByLabel("Choose a HookBox config file to import").setInputFiles({
    name: "config.json",
    mimeType: "application/json",
    buffer: Buffer.from(text, "utf-8"),
  });
}

function validImportBundle() {
  return {
    hookbox_config_version: 1,
    exported_at: "2026-01-01T00:00:00.000Z",
    endpoint: {
      name: "Imported name",
      auto_crud: false,
      target_url: "https://new-target.example.com",
      default_mode: "echo",
      latency_ms: 250,
      rate_limit_per_min: 10,
      chaos_pct: 5,
      chaos_mode: "dropout",
      cors_enabled: true,
    },
    rules: [
      {
        name: "Imported rule",
        priority: 100,
        enabled: true,
        match: {
          method: "GET",
          path: "/imported",
          headers: {},
          query: {},
          body_conditions: [],
          state_requirements: [],
        },
        response: {
          status_code: 200,
          headers: {},
          body_template: "{}",
          content_type: "application/json",
        },
        state_writes: [],
        latency_ms: null,
        rate_limit_per_min: null,
        chaos_mode: null,
        webhook_action: null,
      },
    ],
  };
}

test.describe("F3 export config", () => {
  test("AC-11/12/13/14: downloads hookbox-config-<token>.json with the frozen shape", async ({
    page,
  }) => {
    await installMockBackend(page, {
      authed: true,
      rules: [makeRule(1, { name: "Existing rule" })],
    });
    await page.goto(`/d/${TOKEN}/settings`);
    await expect(page.getByRole("heading", { name: "Settings" })).toBeVisible();

    const downloadPromise = page.waitForEvent("download");
    await page.getByRole("button", { name: "Export config" }).click();
    const download = await downloadPromise;
    expect(download.suggestedFilename()).toBe(`hookbox-config-${TOKEN}.json`);

    const bundle = JSON.parse(await readDownloadText(download));
    expect(bundle.hookbox_config_version).toBe(1);
    expect(bundle.endpoint).not.toHaveProperty("token");
    expect(bundle.endpoint).not.toHaveProperty("mock_url");
    expect(bundle.rules).toHaveLength(1);
    expect(bundle.rules[0]).not.toHaveProperty("id");
    expect(bundle.rules[0]).not.toHaveProperty("token");
    await expect(
      page.getByText("Configuration exported.", { exact: true }),
    ).toBeVisible();
  });

  test("AC-87: export uses freshly fetched server state, not the dirty in-memory form", async ({
    page,
  }) => {
    await installMockBackend(page, { authed: true });
    await page.goto(`/d/${TOKEN}/settings`);
    await page.getByLabel("Endpoint name").fill("Unsaved edit");
    await expect(
      page.getByText(
        "Exports the saved configuration — save your changes first to include them.",
      ),
    ).toBeVisible();

    const downloadPromise = page.waitForEvent("download");
    await page.getByRole("button", { name: "Export config" }).click();
    const download = await downloadPromise;
    const bundle = JSON.parse(await readDownloadText(download));
    expect(bundle.endpoint.name).toBe("Checkout API"); // the SAVED value
  });
});

test.describe("F3 import config — rejections (AC-16), zero network writes", () => {
  async function withRequestCounters(page: Page) {
    let patchCount = 0;
    let postCount = 0;
    await page.route(`**/api/endpoints/${TOKEN}`, async (route) => {
      if (route.request().method() === "PATCH") patchCount++;
      await route.fallback();
    });
    await page.route(`**/api/endpoints/${TOKEN}/rules`, async (route) => {
      if (route.request().method() === "POST") postCount++;
      await route.fallback();
    });
    return {
      get patchCount() {
        return patchCount;
      },
      get postCount() {
        return postCount;
      },
    };
  }

  test("malformed JSON", async ({ page }) => {
    await installMockBackend(page, { authed: true });
    const counters = await withRequestCounters(page);
    await page.goto(`/d/${TOKEN}/settings`);
    await importFile(page, "{not valid json");
    await expect(page.getByText(/isn't valid JSON/)).toBeVisible();
    expect(counters.patchCount).toBe(0);
    expect(counters.postCount).toBe(0);
  });

  test("empty file", async ({ page }) => {
    await installMockBackend(page, { authed: true });
    const counters = await withRequestCounters(page);
    await page.goto(`/d/${TOKEN}/settings`);
    await importFile(page, "");
    await expect(page.getByText("That file is empty.", { exact: true })).toBeVisible();
    expect(counters.patchCount).toBe(0);
    expect(counters.postCount).toBe(0);
  });

  test("wrong version", async ({ page }) => {
    await installMockBackend(page, { authed: true });
    const counters = await withRequestCounters(page);
    await page.goto(`/d/${TOKEN}/settings`);
    await importFile(page, JSON.stringify({ ...validImportBundle(), hookbox_config_version: 2 }));
    await expect(page.getByText(/Unsupported config version 2/)).toBeVisible();
    expect(counters.patchCount).toBe(0);
    expect(counters.postCount).toBe(0);
  });

  test("more than 200 rules", async ({ page }) => {
    await installMockBackend(page, { authed: true });
    const counters = await withRequestCounters(page);
    await page.goto(`/d/${TOKEN}/settings`);
    const bundle = validImportBundle();
    const many = Array.from({ length: 201 }, () => bundle.rules[0]);
    await importFile(page, JSON.stringify({ ...bundle, rules: many }));
    await expect(page.getByText(/201 rules/)).toBeVisible();
    expect(counters.patchCount).toBe(0);
    expect(counters.postCount).toBe(0);
  });

  test("an unknown top-level key is rejected (strict schema)", async ({ page }) => {
    await installMockBackend(page, { authed: true });
    const counters = await withRequestCounters(page);
    await page.goto(`/d/${TOKEN}/settings`);
    await importFile(page, JSON.stringify({ ...validImportBundle(), owner_secret: "sneaky" }));
    await expect(page.getByText(/isn't a HookBox config/)).toBeVisible();
    expect(counters.patchCount).toBe(0);
    expect(counters.postCount).toBe(0);
  });

  test("cancelling the AC-S21 confirm issues zero requests", async ({ page }) => {
    await installMockBackend(page, { authed: true });
    const counters = await withRequestCounters(page);
    await page.goto(`/d/${TOKEN}/settings`);
    await importFile(page, JSON.stringify(validImportBundle()));
    await expect(
      page.getByRole("heading", { name: "Apply this configuration?" }),
    ).toBeVisible();
    await page.getByRole("button", { name: "Cancel" }).click();
    await expect(
      page.getByRole("heading", { name: "Apply this configuration?" }),
    ).toHaveCount(0);
    expect(counters.patchCount).toBe(0);
    expect(counters.postCount).toBe(0);
  });
});

test.describe("F3 import config — apply", () => {
  test("AC-S21/17/18/89: the confirm shows the diff before any request; confirming applies config-then-rules and the form + next Save reflect it", async ({
    page,
  }) => {
    await installMockBackend(page, { authed: true, rules: [] });
    let patchBodies: unknown[] = [];
    const postBodies: unknown[] = [];
    await page.route(`**/api/endpoints/${TOKEN}`, async (route) => {
      if (route.request().method() === "PATCH") {
        patchBodies.push(route.request().postDataJSON());
      }
      await route.fallback();
    });
    await page.route(`**/api/endpoints/${TOKEN}/rules`, async (route) => {
      if (route.request().method() === "POST") {
        postBodies.push(route.request().postDataJSON());
      }
      await route.fallback();
    });
    await page.goto(`/d/${TOKEN}/settings`);

    const bundle = validImportBundle();
    await importFile(page, JSON.stringify(bundle));

    // AC-S21: the confirm shows the diff BEFORE any request.
    await expect(
      page.getByRole("heading", { name: "Apply this configuration?" }),
    ).toBeVisible();
    await expect(page.getByText("target_url", { exact: true })).toBeVisible();
    await expect(
      page.getByText(/target_url changes where unmatched requests are proxied/),
    ).toBeVisible();
    await expect(
      page.getByText("Adds 1 rules to the 0 already on this endpoint."),
    ).toBeVisible();
    expect(patchBodies).toHaveLength(0);
    expect(postBodies).toHaveLength(0);

    await page.getByRole("button", { name: "Apply configuration" }).click();

    // AC-17/18: config first, then one POST per rule, in array order.
    await expect(
      page.getByText("Configuration applied — 1 rules added.", { exact: true }),
    ).toBeVisible();
    expect(patchBodies).toHaveLength(1);
    expect(patchBodies[0]).toMatchObject(bundle.endpoint);
    expect(postBodies).toHaveLength(1);
    expect(postBodies[0]).toMatchObject(bundle.rules[0]);

    // AC-89(a): the rendered form shows the IMPORTED values immediately.
    await expect(page.getByLabel("Endpoint name")).toHaveValue("Imported name");
    await expect(page.getByLabel("Target URL")).toHaveValue(
      "https://new-target.example.com",
    );

    // AC-89(b): a Save clicked immediately after import sends the IMPORTED
    // values, never the pre-import ones.
    patchBodies = [];
    await page.getByRole("button", { name: "Save", exact: true }).click();
    await expect(page.getByText("Settings saved.", { exact: true })).toBeVisible();
    expect(patchBodies).toHaveLength(1);
    expect(patchBodies[0]).toMatchObject({ name: "Imported name" });
  });

  test("AC-19: a partial failure (rule 2 of 2 fails) creates rule 1, never attempts rule 2, and reports all five facts", async ({
    page,
  }) => {
    await installMockBackend(page, { authed: true, rules: [] });
    let ruleCallCount = 0;
    await page.route(`**/api/endpoints/${TOKEN}/rules`, async (route) => {
      if (route.request().method() !== "POST") return route.fallback();
      ruleCallCount++;
      if (ruleCallCount === 2) {
        return route.fulfill({
          status: 422,
          contentType: "application/json",
          body: JSON.stringify({
            error: "validation_error",
            detail: "body_template exceeds the 256 KB cap.",
          }),
        });
      }
      await route.fallback();
    });
    await page.goto(`/d/${TOKEN}/settings`);

    const bundle = validImportBundle();
    const rule2 = { ...bundle.rules[0], name: "Second rule" };
    await importFile(page, JSON.stringify({ ...bundle, rules: [bundle.rules[0], rule2] }));
    await page.getByRole("button", { name: "Apply configuration" }).click();

    await expect(
      page.getByText(
        'Settings were applied and 1 of 2 rules were created. Rule 2 ("Second rule") failed: body_template exceeds the 256 KB cap. No rule after it was attempted, and nothing was rolled back.',
        { exact: true },
      ),
    ).toBeVisible();
    expect(ruleCallCount).toBe(2); // rule 3 (there is none) never attempted
    // The report is persistent, not a 3.2s toast — still there after a wait.
    await page.waitForTimeout(3500);
    await expect(page.getByText("No rule after it was attempted")).toBeVisible();
    await page.getByRole("button", { name: "Dismiss" }).click();
    await expect(page.getByText("No rule after it was attempted")).toHaveCount(0);
  });
});
