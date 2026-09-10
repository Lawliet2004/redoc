import { test, expect } from "@playwright/test";

/**
 * Smoke tests for the Redoc office suite.
 *
 * These verify that the app loads, mode switching works, and basic
 * editor interactions function without crashing.
 */

test.describe("App startup", () => {
  test("home screen loads", async ({ page }) => {
    await page.goto("/");
    await expect(page.locator("text=Redoc")).toBeVisible();
    await expect(page.locator("text=Documents, spreadsheets, presentations")).toBeVisible();
  });

  test("mode tabs are visible", async ({ page }) => {
    await page.goto("/");
    await expect(page.locator("[data-testid='mode-tab-doc']")).toBeVisible();
    await expect(page.locator("[data-testid='mode-tab-sheet']")).toBeVisible();
    await expect(page.locator("[data-testid='mode-tab-slide']")).toBeVisible();
  });
});

test.describe("Document editor", () => {
  test("switches to document editor", async ({ page }) => {
    await page.goto("/");
    await page.locator("[data-testid='mode-tab-doc']").click();
    await expect(page.locator("#editor-pane-doc")).toBeVisible();
  });

  test("can type in document editor", async ({ page }) => {
    await page.goto("/");
    await page.locator("[data-testid='mode-tab-doc']").click();
    const editor = page.locator(".ProseMirror");
    await editor.click();
    await editor.type("Hello, Redoc!");
    await expect(page.locator("text=Hello, Redoc!")).toBeVisible();
  });
});

test.describe("Spreadsheet editor", () => {
  test("switches to spreadsheet editor", async ({ page }) => {
    await page.goto("/");
    await page.locator("[data-testid='mode-tab-sheet']").click();
    await expect(page.locator("#editor-pane-sheet")).toBeVisible();
  });

  test("grid canvas is visible", async ({ page }) => {
    await page.goto("/");
    await page.locator("[data-testid='mode-tab-sheet']").click();
    await expect(page.locator("canvas")).toBeVisible();
  });
});

test.describe("Presentation editor", () => {
  test("switches to presentation editor", async ({ page }) => {
    await page.goto("/");
    await page.locator("[data-testid='mode-tab-slide']").click();
    await expect(page.locator("#editor-pane-slide")).toBeVisible();
  });

  test("slide canvas is visible", async ({ page }) => {
    await page.goto("/");
    await page.locator("[data-testid='mode-tab-slide']").click();
    await expect(page.locator(".slide-stage")).toBeVisible();
  });
});

test.describe("Mode switching", () => {
  test("can switch between all modes", async ({ page }) => {
    await page.goto("/");

    // Doc
    await page.locator("[data-testid='mode-tab-doc']").click();
    await expect(page.locator("#editor-pane-doc")).toBeVisible();

    // Sheet
    await page.locator("[data-testid='mode-tab-sheet']").click();
    await expect(page.locator("#editor-pane-sheet")).toBeVisible();

    // Slide
    await page.locator("[data-testid='mode-tab-slide']").click();
    await expect(page.locator("#editor-pane-slide")).toBeVisible();

    // Back to home
    await page.locator("[data-testid='mode-home']").click();
    await expect(page.locator("text=Redoc")).toBeVisible();
  });
});
