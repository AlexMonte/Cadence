import { defineConfig, devices } from "@playwright/test";

const isCI = process.env.CI === "true";

export default defineConfig({
  testDir: "./e2e",
  fullyParallel: false,
  forbidOnly: isCI,
  retries: isCI ? 2 : 0,
  workers: 1,
  use: {
    baseURL: "http://127.0.0.1:4517",
    trace: "on-first-retry",
  },
  webServer: {
    command: "dx serve --web --port 4517",
    port: 4517,
    reuseExistingServer: !isCI,
    timeout: 120000,
  },
  projects: [
    {
      name: "chromium",
      use: { ...devices["Desktop Chrome"] },
    },
  ],
});
