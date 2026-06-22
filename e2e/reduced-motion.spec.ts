/**
 * Reduced-motion e2e (AC-D16). Runs under the `reduced-motion` project
 * (reducedMotion: 'reduce'). The feed-row arrival animation, skeleton shimmer,
 * and spinners must collapse to a static state — the globals.css
 * prefers-reduced-motion block sets feed-row animation:none, etc. We assert the
 * feed row carries no running animation under the reduced preference.
 */
import { expect, test } from "@playwright/test";
import { installMockBackend, TOKEN } from "./mock-backend";

test("AC-D16: feed-row arrival animation is suppressed under reduced motion", async ({
  page,
}) => {
  // Force the preference at the page level (robust across Playwright versions).
  await page.emulateMedia({ reducedMotion: "reduce" });
  await installMockBackend(page, { authed: true });
  await page.goto(`/d/${TOKEN}`);
  const row = page.getByRole("option").first();
  await expect(row).toBeVisible();

  // The reduced-motion preference must actually be active in this project.
  const prefersReduced = await page.evaluate(
    () => window.matchMedia("(prefers-reduced-motion: reduce)").matches,
  );
  expect(prefersReduced).toBe(true);

  // Motion is neutralized: either the feed-row animation is forced to none, or
  // every animation/transition duration collapses to ~0 (globals.css §reduced).
  const { animName, animDur, transDur } = await row.evaluate((el) => {
    const s = getComputedStyle(el);
    return {
      animName: s.animationName,
      animDur: parseFloat(s.animationDuration) || 0,
      transDur: parseFloat(s.transitionDuration) || 0,
    };
  });
  const neutralized =
    animName === "none" ||
    animName === "" ||
    (animDur <= 0.01 && transDur <= 0.01);
  expect(
    neutralized,
    `animName=${animName} animDur=${animDur} transDur=${transDur}`,
  ).toBeTruthy();
});
