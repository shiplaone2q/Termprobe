import { test as base, expect } from "@playwright/test";
import { copyFile, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { gzipSync } from "node:zlib";
import { attachment, fixture, hostile, query, renderReport, writeReport } from "./fixture.mjs";

const test = base.extend({
  openReport: async ({ page }, use, testInfo) => {
    const errors = [];
    const requests = [];
    page.on("pageerror", (error) => errors.push(error.message));
    page.on("request", (request) => {
      if (!/^(file|data|blob):/.test(request.url())) requests.push(request.url());
    });
    await use(async (data = fixture(), hash = "") => {
      const file = await writeReport(testInfo.outputPath("portable", "failure.html"), data);
      await page.goto("about:blank");
      await page.goto(pathToFileURL(file).href + hash);
      await expect(page.locator("#frame-heading")).toBeVisible();
      await expect(page.locator("#report-error")).toBeHidden();
      return file;
    });
    expect(errors).toEqual([]);
    expect(requests).toEqual([]);
  },
});

async function pauseClock(page) {
  // Installing the clock alone still lets real time elapse between browser actions.
  await page.clock.install({ time: 0 });
  await page.clock.pauseAt(60_000);
}

async function clickCell(page, column, row) {
  const box = await page.locator("#terminal").boundingBox();
  const data = await page.locator("#report-data").textContent();
  const frames = JSON.parse(data).timeline.frames;
  const index = Number(await page.locator("#frame-slider").inputValue());
  const size = frames[index].size;
  // Fit can make cells barely one CSS pixel wide; round the absolute center before Chromium floors click coordinates.
  await page.locator("#terminal").click({ position: {
    x: Math.round(box.x + (column + .5) * box.width / size.cols) - box.x,
    y: Math.round(box.y + (row + .5) * box.height / size.rows) - box.y,
  } });
}

test("the HTML still works alone after its original directory is removed", async ({ page, openReport }, testInfo) => {
  const original = await openReport();
  const portable = testInfo.outputPath("moved", "report with spaces.html");
  await mkdir(path.dirname(portable), { recursive: true });
  await copyFile(original, portable);
  await page.goto("about:blank");
  await rm(path.dirname(original), { recursive: true });
  await page.goto(pathToFileURL(portable).href);
  await expect(page.locator("#operation")).toHaveText("Foreground mismatch");
  await expect(page.locator("#screen-image")).toBeVisible();
  await expect(page.locator("script[src], link[href], iframe, object")).toHaveCount(0);
  await expect(page.locator("#session-summary")).toContainText("deployment-wizard");
  expect(await page.evaluate(() => globalThis.reportPwned)).toBeUndefined();
});

test("failure explanation and highlights use only the decisive locator stage", async ({ page, openReport }) => {
  await openReport();
  await expect(page.locator("#summary")).toContainText("Failed:");
  await expect(page.locator("#summary-observed")).toContainText("#112233");
  await expect(page.locator("#error-locator")).toContainText('getByText("A").getByStyle({ foreground: "2" })');
  await expect(page.locator(".mismatch-cell")).toHaveCount(1);
  await page.locator("#cell-tab").click();
  await expect(page.locator("#cell-mismatches")).toContainText("foreground / stage 1");
  await expect(page.locator("#cell-mismatches")).toContainText("underline_color / stage 1");
  await expect(page.locator("#cell-properties")).toContainText("1 -> #112233");
});

test("passing actions sharing a failure frame do not inherit mismatches", async ({ page, openReport }) => {
  await openReport();
  await page.locator('#points [data-operation="5"]').click();
  await expect(page.locator("#frame-meta")).toContainText("Screen 3 / also used by failure");
  await expect(page.locator("#expectation-banner")).toHaveAttribute("data-outcome", "passed");
  await expect(page.locator("#call-panel")).toBeVisible();
  await expect(page.locator(".mismatch-cell")).toHaveCount(0);
  await expect(page.locator("#cell-mismatches")).toBeHidden();
  await page.locator("#failure").click();
  await expect(page.locator("#errors-panel")).toBeVisible();
  await expect(page.locator(".mismatch-cell")).toHaveCount(1);
});

test("evicted completion checkpoints never substitute a retained frame", async ({ page, openReport }) => {
  await openReport();
  await page.locator('#points [data-operation="1"]').click();
  await expect(page.locator("#frame-meta")).toContainText("Screen 999 was not retained");
  await expect(page.locator("#screen-image")).toHaveCount(0);
  await expect(page.locator("#screen-text")).toContainText("evicted");
  await expect(page.locator("#frame-count")).toHaveText("Unavailable");
  await expect(page.locator("#inspect")).toBeDisabled();
  await expect(page.locator("#play")).toBeDisabled();
  await expect(page.locator("#call-properties")).toContainText("evidence limit");
  await page.locator("#failure").click();
  await expect(page.locator("#frame-count")).toHaveText("3 / 3");
});

test("filters search locator and input text and drive action navigation", async ({ page, openReport }) => {
  await openReport();
  await page.locator("#action-filter").fill('getByText("READY")');
  await expect(page.locator("#points .point")).toHaveCount(2);
  await page.locator("#next-point").click();
  await expect(page.locator('#points [data-operation="2"]')).toHaveAttribute("aria-current", "true");
  await page.locator("#next-point").click();
  await expect(page.locator('#points [data-operation="3"]')).toHaveAttribute("aria-current", "true");
  await expect(page.locator("#next-point")).toBeDisabled();
  await page.locator("#action-filter").fill("DEPLOY staging");
  await expect(page.locator("#points .point")).toHaveCount(1);
  await page.locator("#assertions-only").check();
  await expect(page.locator("#points")).toHaveText("No matching actions.");
  await expect(page.locator("#previous-point")).toBeDisabled();
  await expect(page.locator("#next-point")).toBeDisabled();
  await page.locator("#action-filter").fill("");
  await expect(page.locator("#points .point")).toHaveCount(5);
});

