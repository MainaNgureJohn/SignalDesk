import { defineConfig, devices } from "@playwright/test";

// The SignalDesk shell replaces Buzz's navigation and onboarding screens.
// Keep the upstream suites in playwright.config.ts; the required desktop
// browser check exercises the workflows that this application actually ships.
export default defineConfig({
  testDir: "./tests/e2e",
  timeout: 30_000,
  globalTimeout: 5 * 60_000,
  retries: process.env.CI ? 1 : 0,
  workers: 1,
  reporter: [
    ["line"],
    ["json", { outputFile: "playwright-report.json" }],
    ["html", { open: "never", outputFolder: "playwright-report" }],
  ],
  use: {
    ...devices["Desktop Chrome"],
    ...(process.env.PLAYWRIGHT_BROWSER_CHANNEL
      ? { channel: process.env.PLAYWRIGHT_BROWSER_CHANNEL }
      : {}),
    baseURL: "http://127.0.0.1:4173",
    screenshot: "only-on-failure",
    trace: "retain-on-failure",
    video: "retain-on-failure",
  },
  projects: [{ name: "signaldesk", testMatch: "**/signaldesk*.spec.ts" }],
  webServer: {
    command:
      "node node_modules/vite/bin/vite.js preview --host 127.0.0.1 --port 4173 --strictPort",
    url: "http://127.0.0.1:4173",
    reuseExistingServer: !process.env.CI,
  },
});
