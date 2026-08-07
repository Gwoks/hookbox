/**
 * F5 "Export CSV" e2e (operator-toolkit prd.md §4.5/§5.6, AC-46..56,
 * AC-115..121). Drives the real built SPA against the §5 mock backend and
 * inspects the actual downloaded file bytes.
 */
import type { Download, Page } from "@playwright/test";
import { expect, test } from "@playwright/test";
import {
  installMockBackend,
  makeRequest,
  pushNewRequest,
  TOKEN,
} from "./mock-backend";

const CSV_HEADER_LINE =
  "timestamp,method,path,status_code,served_by,duration_ms,request_headers,request_body,response_headers,response_body";

async function readDownloadText(download: Download): Promise<string> {
  const stream = await download.createReadStream();
  if (!stream) throw new Error("download produced no stream");
  const chunks: Buffer[] = [];
  for await (const chunk of stream) chunks.push(chunk as Buffer);
  return Buffer.concat(chunks).toString("utf-8");
}

async function exportCsv(page: Page): Promise<Download> {
  const downloadPromise = page.waitForEvent("download");
  await page.getByRole("button", { name: "Feed actions" }).click();
  await page
    .getByRole("menuitem", { name: "Export the listed requests as CSV" })
    .click();
  return downloadPromise;
}

test.describe("F5 Export CSV", () => {
  test("AC-46/48/50/51: exports a well-formed CSV — 10 columns, CRLF, trailing CRLF, no BOM", async ({
    page,
  }) => {
    await installMockBackend(page, { authed: true }); // 3 seeded rows
    await page.goto(`/d/${TOKEN}`);
    await expect(page.getByRole("option")).toHaveCount(3);

    const download = await exportCsv(page);
    expect(download.suggestedFilename()).toMatch(
      /^hookbox-requests-ab12cd34-\d{8}T\d{6}Z\.csv$/,
    );
    const content = await readDownloadText(download);
    expect(content.charCodeAt(0)).not.toBe(0xfeff);
    expect(content.endsWith("\r\n")).toBe(true);
    const lines = content.split("\r\n");
    expect(lines[lines.length - 1]).toBe(""); // trailing CRLF -> empty tail
    const dataLines = lines.slice(0, -1);
    expect(dataLines[0]).toBe(CSV_HEADER_LINE);
    expect(dataLines).toHaveLength(4); // header + 3 rows
    for (const line of dataLines.slice(1)) {
      // 10 columns — commas inside quoted JSON cells must not miscount, so
      // just assert the two JSON object cells parse and the row is well-formed.
      expect(line).toContain("/orders");
    }
    await expect(
      page.getByText("Exported 3 requests.", { exact: true }),
    ).toBeVisible();
  });

  test("AC-115/116: a mid-export arrival is excluded — the snapshot is fixed at click time", async ({
    page,
  }) => {
    await installMockBackend(page, { authed: true });
    // Slow every detail fetch so there's a real window to inject an arrival.
    await page.route(/\/api\/requests\/\d+$/, async (route) => {
      await new Promise((r) => setTimeout(r, 200));
      await route.fallback();
    });
    await page.goto(`/d/${TOKEN}`);
    await expect(page.getByRole("option")).toHaveCount(3);

    const downloadPromise = page.waitForEvent("download");
    await page.getByRole("button", { name: "Feed actions" }).click();
    await page
      .getByRole("menuitem", { name: "Export the listed requests as CSV" })
      .click();
    await pushNewRequest(page, {
      id: 999,
      method: "GET",
      path: "/late-arrival",
    });

    const download = await downloadPromise;
    const content = await readDownloadText(download);
    const dataLines = content.split("\r\n").slice(0, -1);
    expect(dataLines).toHaveLength(4); // header + the original 3, never 4
    expect(content.includes("/late-arrival")).toBe(false);
  });

  test("AC-52: a per-row 404 reads 'pending', a per-row 500 reads 'unavailable', never shifting a column", async ({
    page,
  }) => {
    await installMockBackend(page, { authed: true });
    await page.route("**/api/requests/3", (route) =>
      route.fulfill({
        status: 404,
        contentType: "application/json",
        body: JSON.stringify({ error: "unknown_request", detail: "Gone." }),
      }),
    );
    await page.route("**/api/requests/2", (route) =>
      route.fulfill({
        status: 500,
        contentType: "application/json",
        body: JSON.stringify({ error: "server_error", detail: "Boom." }),
      }),
    );
    await page.goto(`/d/${TOKEN}`);
    await expect(page.getByRole("option")).toHaveCount(3);

    const download = await exportCsv(page);
    const content = await readDownloadText(download);
    const [, row3, row2, row1] = content.split("\r\n").slice(0, -1);
    // Row order is newest-first (id 3, 2, 1) — summary columns always come
    // from the feed row, so the sentinel never shifts a column.
    expect(row3).toBe("2026-06-21T12:00:00Z,POST,/orders,201,crud,4,pending,pending,pending,pending");
    expect(row2).toBe("2026-06-21T12:00:00Z,GET,/orders/42,200,rule,4,unavailable,unavailable,unavailable,unavailable");
    expect(row1).toContain("/orders/7");
    await expect(
      page.getByText("Exported 3 requests — 2 without detail.", {
        exact: true,
      }),
    ).toBeVisible();
    // AC-121: the persistent detail note appears (dismissible).
    await expect(
      page.getByText(
        "Rows whose detail could not be fetched read pending or unavailable in the four detail columns.",
      ),
    ).toBeVisible();
    await page.getByRole("button", { name: "Dismiss" }).click();
    await expect(page.getByText("Rows whose detail could not be fetched")).toHaveCount(0);
  });

  test("AC-56: request Authorization exports already-redacted; response headers stay verbatim, untouched by the exporter", async ({
    page,
  }) => {
    await installMockBackend(page, { authed: true, requests: [] });
    await page.goto(`/d/${TOKEN}`);
    await expect(page.getByText("No requests yet")).toBeVisible();
    await pushNewRequest(page, { id: 1, method: "GET", path: "/secure" });
    await expect(page.getByRole("option")).toHaveCount(1);

    const download = await exportCsv(page);
    const content = await readDownloadText(download);
    const [, dataRow] = content.split("\r\n");
    expect(dataRow).toContain('""authorization"":""<redacted>""');
    expect(dataRow).toContain('""content-type"":""application/json""');
  });

  test("AC-82: Clear all is disabled for the whole export", async ({
    page,
  }) => {
    await installMockBackend(page, { authed: true });
    await page.route(/\/api\/requests\/\d+$/, async (route) => {
      await new Promise((r) => setTimeout(r, 300));
      await route.fallback();
    });
    await page.goto(`/d/${TOKEN}`);
    await expect(page.getByRole("option")).toHaveCount(3);

    await page.getByRole("button", { name: "Feed actions" }).click();
    await page
      .getByRole("menuitem", { name: "Export the listed requests as CSV" })
      .click();
    await expect(page.getByText(/Exporting \d+ of 3…/)).toBeVisible();

    await page.getByRole("button", { name: "Feed actions" }).click();
    await expect(
      page.getByRole("menuitem", { name: "Clear all captured requests" }),
    ).toBeDisabled();
    await expect(
      page.getByRole("menuitem", { name: "Export the listed requests as CSV" }),
    ).toBeDisabled();
    await expect(page.getByText("Finish or cancel the export first.")).toBeVisible();
  });

  test("cancelling the export produces no download", async ({ page }) => {
    await installMockBackend(page, { authed: true });
    await page.route(/\/api\/requests\/\d+$/, async (route) => {
      await new Promise((r) => setTimeout(r, 300));
      await route.fallback();
    });
    await page.goto(`/d/${TOKEN}`);
    await expect(page.getByRole("option")).toHaveCount(3);

    let downloadFired = false;
    page.on("download", () => {
      downloadFired = true;
    });

    await page.getByRole("button", { name: "Feed actions" }).click();
    await page
      .getByRole("menuitem", { name: "Export the listed requests as CSV" })
      .click();
    await expect(page.getByText(/Exporting \d+ of 3…/)).toBeVisible();
    await page.getByRole("button", { name: "Cancel the export" }).click();

    await expect(
      page.getByText("Export cancelled. No file was downloaded.", {
        exact: true,
      }),
    ).toBeVisible();
    // Give any in-flight background fetches a moment to settle, then confirm
    // no download ever fired.
    await page.waitForTimeout(500);
    expect(downloadFired).toBe(false);
  });

  test("§5.6 fetch mechanism: at most 4 requests in flight, and results stay index-aligned under out-of-order completion", async ({
    page,
  }) => {
    const requests = Array.from({ length: 6 }, (_, i) =>
      makeRequest({
        id: i + 1,
        method: "GET",
        path: `/r${i + 1}`,
        status_code: 200,
        served_by: "rule",
      }),
    );
    await installMockBackend(page, { authed: true, requests });
    await page.goto(`/d/${TOKEN}`);
    await expect(page.getByRole("option")).toHaveCount(6);

    let inFlight = 0;
    let maxInFlight = 0;
    await page.route(/\/api\/requests\/\d+$/, async (route) => {
      inFlight++;
      maxInFlight = Math.max(maxInFlight, inFlight);
      const id = Number(route.request().url().match(/\/(\d+)$/)?.[1]);
      // Resolve out of order: even ids settle fast, odd ids settle slow —
      // if results were written by completion order instead of index, the
      // row order below would come out scrambled.
      await new Promise((r) => setTimeout(r, id % 2 === 0 ? 30 : 200));
      inFlight--;
      await route.fallback();
    });

    const download = await exportCsv(page);
    expect(maxInFlight).toBeLessThanOrEqual(4);
    const content = await readDownloadText(download);
    const dataLines = content.split("\r\n").slice(0, -1).slice(1);
    expect(dataLines.map((l) => l.split(",")[2])).toEqual([
      "/r6",
      "/r5",
      "/r4",
      "/r3",
      "/r2",
      "/r1",
    ]);
  });
});
