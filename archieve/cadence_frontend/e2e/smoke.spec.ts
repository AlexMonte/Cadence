import { expect, test, type Locator, type Page } from "@playwright/test";

const WORKSPACE_STORAGE_DB = "cadence.browser.storage";
const WORKSPACE_STORAGE_STORE = "kv";
const SAMPLE_CACHE_DB = "cadence.sample.cache";
const SAMPLE_CACHE_STORE = "samples";
const WORKSPACE_LOCATION_KEY = "cadence.workspace.location";
const RECOVERY_PAYLOAD_KEY = "cadence.recovery.payload";
const FATAL_WASM_PATTERNS = [
  /time not implemented on this platform/i,
  /thread '.*' panicked/i,
  /\bpanicked at\b/i,
  /RuntimeError:\s+unreachable/i,
];

function installBrowserGuard(page: Page) {
  const pageErrors: string[] = [];
  const panicConsole: string[] = [];

  page.on("pageerror", (error) => {
    pageErrors.push(String(error));
  });
  page.on("console", (message) => {
    const text = message.text();
    if (FATAL_WASM_PATTERNS.some((pattern) => pattern.test(text))) {
      panicConsole.push(`[${message.type()}] ${text}`);
    }
  });

  return {
    async assertClean() {
      expect(
        pageErrors,
        `unexpected browser page errors:\n${pageErrors.join("\n")}`,
      ).toEqual([]);
      expect(
        panicConsole,
        `fatal wasm panic console output:\n${panicConsole.join("\n")}`,
      ).toEqual([]);
    },
  };
}

async function readIndexedDbValue<T>(
  page: Page,
  dbName: string,
  storeName: string,
  key: string,
): Promise<T | null> {
  return page.evaluate(
    async ({ dbName, storeName, key }) => {
      const db = await new Promise<IDBDatabase>((resolve, reject) => {
        const request = indexedDB.open(dbName, 1);
        request.onupgradeneeded = () => {
          const database = request.result;
          if (!database.objectStoreNames.contains(storeName)) {
            database.createObjectStore(storeName);
          }
        };
        request.onsuccess = () => resolve(request.result);
        request.onerror = () =>
          reject(request.error ?? new Error(`failed to open ${dbName}`));
      });

      const tx = db.transaction(storeName, "readonly");
      const store = tx.objectStore(storeName);
      const value = await new Promise<unknown>((resolve, reject) => {
        const request = store.get(key);
        request.onsuccess = () => resolve(request.result ?? null);
        request.onerror = () =>
          reject(request.error ?? new Error(`failed to read ${key}`));
      });

      await new Promise<void>((resolve, reject) => {
        tx.oncomplete = () => resolve();
        tx.onabort = () =>
          reject(tx.error ?? new Error(`transaction aborted for ${dbName}`));
        tx.onerror = () =>
          reject(tx.error ?? new Error(`transaction failed for ${dbName}`));
      });

      db.close();
      return value as T | null;
    },
    { dbName, storeName, key },
  );
}

async function countIndexedDbEntries(
  page: Page,
  dbName: string,
  storeName: string,
): Promise<number> {
  return page.evaluate(
    async ({ dbName, storeName }) => {
      const db = await new Promise<IDBDatabase>((resolve, reject) => {
        const request = indexedDB.open(dbName, 1);
        request.onupgradeneeded = () => {
          const database = request.result;
          if (!database.objectStoreNames.contains(storeName)) {
            database.createObjectStore(storeName);
          }
        };
        request.onsuccess = () => resolve(request.result);
        request.onerror = () =>
          reject(request.error ?? new Error(`failed to open ${dbName}`));
      });

      const tx = db.transaction(storeName, "readonly");
      const store = tx.objectStore(storeName);
      const count = await new Promise<number>((resolve, reject) => {
        const request = store.count();
        request.onsuccess = () => resolve(request.result ?? 0);
        request.onerror = () =>
          reject(request.error ?? new Error(`failed to count ${storeName}`));
      });

      await new Promise<void>((resolve, reject) => {
        tx.oncomplete = () => resolve();
        tx.onabort = () =>
          reject(tx.error ?? new Error(`transaction aborted for ${dbName}`));
        tx.onerror = () =>
          reject(tx.error ?? new Error(`transaction failed for ${dbName}`));
      });

      db.close();
      return count;
    },
    { dbName, storeName },
  );
}

async function gotoEditor(page: Page) {
  await page.goto("/", { waitUntil: "domcontentloaded" });
  await expect(page.locator("#project-name-input")).toBeVisible({ timeout: 120_000 });
  await expect(page.locator("#status-source")).toHaveText("LIVE BACKEND");
  await expect(page.locator("#play-toggle")).toBeEnabled();
  await expect(page.locator("#grid-piece-picker")).toBeVisible();
  await expect(page.locator("#side-config")).toBeVisible();
  await expect
    .poll(() =>
      page.evaluate(() => ({
        isolated: window.crossOriginIsolated,
        sharedArrayBuffer: typeof SharedArrayBuffer === "function",
      })),
    )
    .toEqual({
      isolated: true,
      sharedArrayBuffer: true,
    });
}

async function playRuntime(page: Page) {
  await page.locator("#play-toggle").click();
  await expect(page.locator("#status-host")).toHaveText("Audio ready");
  await expect(page.locator("#status-playback")).toHaveText("Playing");
}

async function stopRuntime(page: Page) {
  await page.locator("#stop-toggle").click();
  await expect
    .poll(async () => (await page.locator("#status-playback").textContent()) ?? "")
    .toMatch(/^(Idle|Ready to play)$/);
}

async function renameProject(page: Page, name: string) {
  await page.locator("#project-name-input").fill(name);
  await page.locator("#cpm-value").click();
  await expect(page.locator("#project-name-input")).toHaveValue(name);
}

async function waitForRecoverySnapshot(page: Page) {
  await expect
    .poll(async () => {
      const payload = await readIndexedDbValue<string>(
        page,
        WORKSPACE_STORAGE_DB,
        WORKSPACE_STORAGE_STORE,
        RECOVERY_PAYLOAD_KEY,
      );
      return typeof payload === "string" && payload.trim().length > 0;
    })
    .toBe(true);
}

async function waitForRecoveryToClear(page: Page) {
  await expect
    .poll(async () => {
      const payload = await readIndexedDbValue<string>(
        page,
        WORKSPACE_STORAGE_DB,
        WORKSPACE_STORAGE_STORE,
        RECOVERY_PAYLOAD_KEY,
      );
      return payload;
    })
    .toBeNull();
}

async function saveProject(page: Page, expectDownload: boolean) {
  if (expectDownload) {
    await Promise.all([
      page.waitForEvent("download"),
      page.locator("#save-project").click(),
    ]);
    return;
  }
  await page.locator("#save-project").click();
}

async function placePiece(page: Page, pieceId: string, query: string) {
  const librarySearch = page.locator("#grid-piece-picker-search");
  if (!(await librarySearch.isVisible())) {
    await page.locator("#add-node").click();
  }
  await librarySearch.fill(query);
  await page.locator(`.grid-piece-picker-item[data-piece-id="${pieceId}"]`).click();
}

async function dragLocatorToLocator(
  page: Page,
  source: Locator,
  target: Locator,
) {
  const sourceBox = await source.boundingBox();
  const targetBox = await target.boundingBox();
  if (!sourceBox || !targetBox) {
    throw new Error("missing drag geometry");
  }
  await page.mouse.move(
    sourceBox.x + sourceBox.width / 2,
    sourceBox.y + sourceBox.height / 2,
  );
  await page.mouse.down();
  await page.mouse.move(
    sourceBox.x + sourceBox.width / 2 + 12,
    sourceBox.y + sourceBox.height / 2 + 4,
    { steps: 4 },
  );
  await page.mouse.move(
    targetBox.x + targetBox.width / 2,
    targetBox.y + targetBox.height / 2,
    { steps: 10 },
  );
  await page.mouse.up();
}

test("web runtime survives first run, save/reload, and sample cache reuse", async ({
  page,
}) => {
  const guard = installBrowserGuard(page);

  await gotoEditor(page);

  await playRuntime(page);
  await expect
    .poll(() => countIndexedDbEntries(page, SAMPLE_CACHE_DB, SAMPLE_CACHE_STORE))
    .toBeGreaterThan(0);
  await stopRuntime(page);

  await renameProject(page, "Smoke IndexedDB");
  await waitForRecoverySnapshot(page);

  await saveProject(page, true);
  await expect
    .poll(() =>
      readIndexedDbValue<string>(
        page,
        WORKSPACE_STORAGE_DB,
        WORKSPACE_STORAGE_STORE,
        WORKSPACE_LOCATION_KEY,
      ),
    )
    .toBe("browser://workspace/project.cadence.json");
  await waitForRecoveryToClear(page);
  await expect(page.locator("#unsaved-modal")).not.toBeVisible();

  await renameProject(page, "Smoke IndexedDB Again");
  await waitForRecoverySnapshot(page);

  await saveProject(page, false);
  await expect
    .poll(() =>
      readIndexedDbValue<string>(
        page,
        WORKSPACE_STORAGE_DB,
        WORKSPACE_STORAGE_STORE,
        WORKSPACE_LOCATION_KEY,
      ),
    )
    .toBe("browser://workspace/project.cadence.json");
  await waitForRecoveryToClear(page);

  await page.reload();
  await expect(page.locator("#project-name-input")).toHaveValue("Smoke IndexedDB Again");
  await expect(page.locator("#unsaved-modal")).not.toBeVisible();
  await expect
    .poll(() => countIndexedDbEntries(page, SAMPLE_CACHE_DB, SAMPLE_CACHE_STORE))
    .toBeGreaterThan(0);

  await playRuntime(page);
  await page.locator("#compile-toggle").click();
  await page.getByRole("button", { name: "Diagnostics" }).click();
  await page.getByRole("button", { name: "Show engine details" }).click();
  await expect(page.locator("#activity-drawer")).toContainText("Cache hits:");
  await stopRuntime(page);

  await guard.assertClean();
});

test("semantic shell keeps core panels and quick actions accessible", async ({
  page,
}) => {
  const guard = installBrowserGuard(page);

  await gotoEditor(page);

  await expect(page.locator("#workspace-runtime")).toHaveText("Song");
  await expect(page.locator("#workspace-init")).toHaveText("Setup");
  await expect(page.locator("#side-config-title")).toHaveText("Selection");

  const librarySearch = page.locator("#grid-piece-picker-search");
  if (!(await librarySearch.isVisible())) {
    await page.locator("#add-node").click();
  }
  await expect(librarySearch).toBeVisible();
  await librarySearch.fill("gain");
  await expect(page.locator("#grid-piece-picker-list")).toContainText("gain");

  await page.locator("#compile-toggle").click();
  await expect(page.locator("#activity-drawer")).toHaveClass(/is-open/);
  await expect(page.getByRole("button", { name: "Issues" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Preview", exact: true })).toBeVisible();
  await expect(page.getByRole("button", { name: "Diagnostics", exact: true })).toBeVisible();

  const primaryModifier = process.platform === "darwin" ? "Meta" : "Control";
  await page.keyboard.press(`${primaryModifier}+K`);
  await expect(page.locator("#command-palette")).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.locator("#command-palette")).not.toBeVisible();

  await guard.assertClean();
});

test("control input tiles open the modal editor with numeric shape controls", async ({
  page,
}) => {
  const guard = installBrowserGuard(page);

  await gotoEditor(page);
  await placePiece(page, "cadence.control_input", "control input");

  const controlNodeShell = page
    .locator('.voice-node-shell:has(.voice-node[data-piece-id="cadence.control_input"])')
    .last();
  await expect(controlNodeShell).toBeVisible();
  await controlNodeShell.locator(".node-input-control").click();

  await expect(page.locator("#input-editor-modal")).toBeVisible();
  await expect(page.locator("#input-editor-modal")).toContainText("Control Input");
  await expect(page.locator("#input-editor-modal")).toContainText("Widget");
  await expect(page.locator("#input-editor-modal")).toContainText("Range");
  await expect(page.locator("#input-editor-modal")).toContainText("Min");
  await expect(page.locator("#input-editor-modal")).toContainText("Max");
  await expect(page.locator("#input-editor-modal")).toContainText("Step");
  await expect(page.locator("#input-editor-modal .input-editor-range")).toBeVisible();
  await expect(page.locator("#input-editor-modal .input-editor-number").first()).toBeVisible();

  await guard.assertClean();
});

test("tile body drag moves tiles across the canvas", async ({ page }) => {
  const guard = installBrowserGuard(page);

  await gotoEditor(page);
  await placePiece(page, "cadence.rev", "rev");

  const node = page.locator('.canvas-layer .voice-node[data-piece-id="cadence.rev"]').first();
  const sourcePos = await node.getAttribute("data-grid-pos");
  if (!sourcePos) {
    throw new Error("missing source grid position");
  }
  const targetCell = page.locator('.grid-cell:not(.is-occupied)').first();
  const targetPos = await targetCell.getAttribute("data-grid-pos");
  if (!targetPos) {
    throw new Error("missing target grid position");
  }

  await expect(node).toBeVisible();
  await dragLocatorToLocator(page, node, targetCell);

  await expect(node).toHaveAttribute("data-grid-pos", targetPos);

  await guard.assertClean();
});

test("tile body click selects without moving", async ({ page }) => {
  const guard = installBrowserGuard(page);

  await gotoEditor(page);
  await placePiece(page, "cadence.rev", "rev");

  const node = page.locator('.canvas-layer .voice-node[data-piece-id="cadence.rev"]').first();
  const sourcePos = await node.getAttribute("data-grid-pos");
  if (!sourcePos) {
    throw new Error("missing source grid position");
  }
  const [sourceCol, sourceRow] = sourcePos.split(":").map((value) => Number.parseInt(value, 10));

  await node.click();

  await expect(node).toHaveAttribute("data-grid-pos", sourcePos);
  await expect(page.locator("#status-selection")).toContainText("rev");
  await expect(page.locator(`.grid-cell[data-grid-pos="${sourceCol}:${sourceRow}"]`)).toHaveClass(
    /is-selected/,
  );

  await guard.assertClean();
});

test("control dial drag updates the authored value", async ({ page }) => {
  const guard = installBrowserGuard(page);

  await gotoEditor(page);
  await placePiece(page, "cadence.control_input", "control input");

  const shell = page
    .locator('.voice-node-shell:has(.voice-node[data-piece-id="cadence.control_input"])')
    .first();
  const dial = shell.locator(".dial-shell");
  const value = shell.locator(".node-control-widget-value").first();

  await expect(dial).toBeVisible();
  await expect(value).toHaveText("0");

  const dialBox = await dial.boundingBox();
  if (!dialBox) {
    throw new Error("missing dial geometry");
  }

  await page.mouse.move(dialBox.x + dialBox.width / 2, dialBox.y + dialBox.height / 2);
  await page.mouse.down();
  await page.mouse.move(dialBox.x + dialBox.width / 2, dialBox.y + dialBox.height / 2 - 32, {
    steps: 8,
  });
  await page.mouse.up();

  await expect(value).not.toHaveText("0");

  await guard.assertClean();
});
