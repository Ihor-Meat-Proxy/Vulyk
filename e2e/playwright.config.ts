import { defineConfig, devices } from '@playwright/test'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const __dirname = path.dirname(fileURLToPath(import.meta.url))
const baseURL = process.env.E2E_BASE_URL ?? 'http://127.0.0.1:5173'
const artifactsDir = path.join(__dirname, 'artifacts')
const htmlReportDir =
  process.env.PLAYWRIGHT_HTML_REPORT ?? path.join(artifactsDir, 'html-report', 'latest')

export default defineConfig({
  testDir: './tests',
  outputDir: path.join(artifactsDir, 'test-results'),
  fullyParallel: false,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  workers: 1,
  timeout: 120_000,
  expect: { timeout: 20_000 },
  // Vulyk reporter is a no-op unless VULYK_URL + VULYK_CI_TOKEN + VULYK_PROJECT are set.
  reporter: [
    ['list'],
    ['html', { open: 'never', outputFolder: htmlReportDir }],
    ['./reporters/vulyk-reporter.ts'],
  ],
  globalSetup: path.join(__dirname, 'global-setup.ts'),
  globalTeardown: path.join(__dirname, 'global-teardown.ts'),
  use: {
    baseURL,
    // Local Docker / flake triage: keep traces on failure; CI still retries once.
    trace: process.env.CI ? 'on-first-retry' : 'retain-on-failure',
    screenshot: 'only-on-failure',
    video: 'off',
  },
  projects: [
    {
      name: 'chromium',
      testIgnore: /pwa-mobile\.spec\.ts/,
      use: { ...devices['Desktop Chrome'] },
    },
    {
      // Pixel 5 viewport on Chromium — no WebKit/Firefox.
      name: 'chromium-mobile',
      testMatch: /pwa-mobile\.spec\.ts/,
      use: {
        ...devices['Desktop Chrome'],
        ...devices['Pixel 5'],
        browserName: 'chromium',
      },
    },
  ],
})
