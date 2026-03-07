import path from "node:path";
import { fileURLToPath } from "node:url";

import { expect, test } from "@playwright/test";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const mockScript = path.resolve(__dirname, "helpers/mock-tauri.js");

function gridCell(page, col, row) {
  return page.locator(`.grid-cell[data-grid-pos="${col}:${row}"]`);
}

function nodeAt(page, col, row) {
  return page.locator(`.voice-node[data-grid-pos="${col}:${row}"]`);
}

function pickerItem(page, pieceId) {
  return page.locator(`.grid-piece-picker-item[data-piece-id="${pieceId}"]`);
}

async function openPickerFromToolbar(page) {
  await page.locator("#add-node").click();
  await expect(page.locator("#grid-piece-picker")).toBeVisible();
}

async function openPickerAtCell(page, col, row) {
  await gridCell(page, col, row).click({ button: "right" });
  await expect(page.locator("#grid-piece-picker")).toBeVisible();
}

async function placeFromPicker(page, pieceId) {
  await pickerItem(page, pieceId).click();
}

async function dragPointer(page, source, target, options = {}) {
  const sourceOffset = options.sourceOffset ?? { x: 0.5, y: 0.5 };
  const targetOffset = options.targetOffset ?? { x: 0.5, y: 0.5 };
  const release = options.release ?? true;

  const sourceBox = await source.boundingBox();
  const targetBox = await target.boundingBox();
  if (!sourceBox || !targetBox) {
    throw new Error("Missing source or target box for pointer drag.");
  }

  const startX = sourceBox.x + sourceBox.width * sourceOffset.x;
  const startY = sourceBox.y + sourceBox.height * sourceOffset.y;
  const endX = targetBox.x + targetBox.width * targetOffset.x;
  const endY = targetBox.y + targetBox.height * targetOffset.y;

  await page.mouse.move(startX, startY);
  await page.mouse.down();
  await page.mouse.move(startX + 10, startY + 10, { steps: 2 });
  await page.mouse.move(endX, endY, { steps: 4 });
  if (release) {
    await page.mouse.up();
  }
}

test.beforeEach(async ({ page }) => {
  await page.addInitScript({ path: mockScript });
  await page.goto("/");
  await expect(page.locator(".grid-cell").first()).toBeVisible();
});

test("add-node and picker insertion both place tiles", async ({ page }) => {
  await openPickerFromToolbar(page);
  await placeFromPicker(page, "strudel.sound");
  await expect(nodeAt(page, 0, 0)).toHaveCount(1);

  await openPickerAtCell(page, 1, 0);
  await placeFromPicker(page, "strudel.output");
  await expect(nodeAt(page, 1, 0)).toHaveCount(1);
});

test("pointer drag places, moves, and connects tiles", async ({ page }) => {
  await openPickerFromToolbar(page);
  await placeFromPicker(page, "strudel.sound");
  await expect(nodeAt(page, 0, 0)).toHaveCount(1);

  await openPickerFromToolbar(page);
  await placeFromPicker(page, "strudel.output");
  await expect(nodeAt(page, 1, 0)).toHaveCount(1);

  await openPickerFromToolbar(page);
  await dragPointer(page, pickerItem(page, "strudel.fast"), gridCell(page, 2, 0));
  await expect(nodeAt(page, 2, 0)).toHaveCount(1);

  await dragPointer(page, nodeAt(page, 2, 0), gridCell(page, 2, 1));
  await expect(nodeAt(page, 2, 1)).toHaveCount(1);

  await dragPointer(page, nodeAt(page, 0, 0).locator(".node-output-handle"), nodeAt(page, 1, 0));
  await expect(page.locator(".graph-edge-line")).toHaveCount(1);
  await expect(nodeAt(page, 0, 0)).toHaveAttribute("data-piece-id", "strudel.sound");
  await expect(nodeAt(page, 1, 0)).toHaveAttribute("data-piece-id", "strudel.output");
});

test("drag hover highlights only hovered slot before drop", async ({ page }) => {
  await openPickerAtCell(page, 0, 0);
  const source = pickerItem(page, "strudel.sound");
  const target = gridCell(page, 0, 0);
  const other = gridCell(page, 1, 0);

  await dragPointer(page, source, target, { release: false });

  await expect(target).toHaveClass(/is-drop-valid/);
  await expect(other).not.toHaveClass(/is-drop-valid|is-drop-invalid|is-move-valid|is-move-swap|is-move-invalid/);

  await page.mouse.up();
  await expect(nodeAt(page, 0, 0)).toHaveCount(1);
});

test("grid background origin stays aligned with slot origin", async ({ page }) => {
  const gridBgPos = await page.locator("#canvas-grid").evaluate((el) => getComputedStyle(el).backgroundPosition);
  expect(gridBgPos).toContain("32px 32px");

  const firstCellOrigin = await gridCell(page, 0, 0).evaluate((el) => ({
    left: getComputedStyle(el).left,
    top: getComputedStyle(el).top,
  }));
  expect(firstCellOrigin.left).toBe("32px");
  expect(firstCellOrigin.top).toBe("32px");
});

test("grid overflow, picker, and tile modal remain accessible", async ({ page }) => {
  await page.locator("#canvas").evaluate((el) => {
    el.scrollLeft = el.scrollWidth;
    el.scrollTop = el.scrollHeight;
  });

  await openPickerAtCell(page, 8, 8);
  await placeFromPicker(page, "strudel.sound");
  await expect(nodeAt(page, 8, 8)).toHaveCount(1);

  await openPickerAtCell(page, 7, 8);
  await placeFromPicker(page, "strudel.output");
  await expect(nodeAt(page, 7, 8)).toHaveCount(1);

  await nodeAt(page, 8, 8).click();
  await expect(page.locator("#tile-inspector-modal")).toBeVisible();
  const dataScroll = page.locator(".tile-data-scroll").first();
  await expect(dataScroll).toBeVisible();
  const overflowY = await dataScroll.evaluate((el) => getComputedStyle(el).overflowY);
  expect(overflowY).toBe("auto");
});

test("init workspace can create and open a trick, then expose it in the runtime palette", async ({ page }) => {
  await page.locator("#workspace-init").click();
  await expect(page.locator("#init-workspace")).toBeVisible();

  await page.locator("#init-trick-name").fill("melodia");
  await page.locator("#init-trick-create").click();
  await expect(page.locator("#init-trick-list")).toContainText("melodia");

  await page.locator("#init-trick-list").getByRole("button", { name: "Open" }).click();
  await expect(page.locator("#workspace-back")).toBeVisible();
  await expect(page.locator("#grid-window")).toBeVisible();

  await openPickerFromToolbar(page);
  await placeFromPicker(page, "cadence.trick_input_1");
  await expect(nodeAt(page, 0, 0)).toHaveCount(1);

  await openPickerAtCell(page, 1, 0);
  await placeFromPicker(page, "cadence.trick_output");
  await expect(nodeAt(page, 1, 0)).toHaveCount(1);

  await page.locator("#workspace-runtime").click();
  await openPickerFromToolbar(page);
  await expect(page.locator(".grid-piece-picker-item").filter({ hasText: "melodia / cadence.trick." }).first()).toBeVisible();
});

test("opening a project resets editor context and reloads the runtime catalog", async ({ page }) => {
  await page.locator("#workspace-init").click();
  await expect(page.locator("#init-workspace")).toBeVisible();

  await page.evaluate(() => window.__CADENCE_MENU_ACTION("file.open"));

  await expect(page.locator("#grid-window")).toBeVisible();
  await expect(page.locator("#init-workspace")).toBeHidden();
  await expect(page.locator("#project-chip")).toContainText("Loaded");
  await expect(nodeAt(page, 0, 0)).toHaveAttribute("data-piece-id", "cadence.trick.loaded_melodia");

  await openPickerFromToolbar(page);
  await expect(page.locator(".grid-piece-picker-item").filter({ hasText: "loaded melodia / cadence.trick.loaded_melodia" })).toBeVisible();
});
