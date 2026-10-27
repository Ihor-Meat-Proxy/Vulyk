import { defineConfig, devices } from '@playwright/test'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const __dirname = path.dirname(fileURLToPath(import.meta.url))
const baseURL = process.env.E2E_BASE_URL ?? 'http://127.0.0.1:5173'

export default defineConfig({
  testDir: './tests',
  fullyParallel: false,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  workers: 1,
  timeout: 120_000,
  expect: { timeout: 20_000 },
  // Vulyk reporter is a no-op unless VULYK_URL + VULYK_CI_TOKEN + VULYK_PROJECT are set.
  reporter: process.env.CI
    ? [['list'], ['html', { open: 'never' }], ['./reporters/vulyk-reporter.ts']]
    : [['list'], ['./reporters/vulyk-reporter.ts']],
  globalSetup: path.join(__dirname, 'global-setup.ts'),
  globalTeardown: path.join(__dirname, 'global-teardown.ts'),
  use: {
    baseURL,
    trace: 'on-first-retry',
    screenshot: 'only-on-failure',
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
