import { expect, test } from '@playwright/test'
import { API_URL } from '../fixtures/env'

test('API docs and openapi.json are self-hosted', {
  tag: ['@VQA-329', '@VUE-105'],
  annotation: [
    { type: 'vulyk', description: 'VQA-329' },
    { type: 'vulyk', description: 'VUE-105' },
  ],
}, async ({ request, page }) => {
  const spec = await request.get(`${API_URL}/api/openapi.json`)
  expect(spec.ok()).toBeTruthy()
  const body = await spec.json()
  expect(body.openapi).toBeTruthy()
  expect(body.paths).toBeTruthy()
  expect(Object.keys(body.paths).length).toBeGreaterThan(10)

  const docs = await request.get(`${API_URL}/api/docs`)
  expect(docs.ok()).toBeTruthy()
  const html = await docs.text()
  expect(html).toContain('/api/docs/assets/scalar.js')
  expect(html).not.toContain('cdn.jsdelivr.net')
  expect(html).not.toContain('https://')

  const js = await request.get(`${API_URL}/api/docs/assets/scalar.js`)
  expect(js.ok()).toBeTruthy()
  const ct = js.headers()['content-type'] ?? ''
  expect(ct).toMatch(/javascript/)

  // Browser load: Scalar must boot without CDN / CSP console errors.
  const consoleErrors: string[] = []
  page.on('console', (msg) => {
    if (msg.type() === 'error') consoleErrors.push(msg.text())
  })
  page.on('pageerror', (err) => consoleErrors.push(String(err)))
  await page.goto(`${API_URL}/api/docs`, { waitUntil: 'networkidle' })
  await expect(page.locator('#api-reference')).toBeAttached()
  // Scalar mounts interactive UI into the body after the standalone script runs.
  await expect
    .poll(async () => page.locator('[class*="scalar"], [data-overlay], .scalar-app, #app').count(), {
      timeout: 15_000,
    })
    .toBeGreaterThan(0)
  const cspBlocked = consoleErrors.filter(
    (e) =>
      /Content Security Policy|cdn\.jsdelivr|Refused to load/i.test(e),
  )
  expect(cspBlocked, `CSP/CDN errors: ${cspBlocked.join('\\n')}`).toEqual([])
})

test('API docs Scalar search, Try it, and Auth stay interactive without CDN', {
  tag: ['@VQA-359', '@VUE-221'],
  annotation: [
    { type: 'vulyk', description: 'VQA-359' },
    { type: 'vulyk', description: 'VUE-221' },
  ],
}, async ({ request, page }) => {
  const docs = await request.get(`${API_URL}/api/docs`)
  expect(docs.ok()).toBeTruthy()
  const html = await docs.text()
  expect(html).toContain('"hideSearch":false')
  expect(html).toContain('"hideTestRequestButton":false')
  expect(html).toContain('"preferredSecurityScheme":"bearer_auth"')
  expect(html).toContain('"withDefaultFonts":false')
  expect(html).not.toContain('fonts.scalar.com')
  expect(html).not.toContain('cdn.jsdelivr.net')
  expect(html).not.toContain('https://')

  const spec = await request.get(`${API_URL}/api/openapi.json`)
  expect(spec.ok()).toBeTruthy()
  const body = await spec.json()
  expect(body.servers?.[0]?.url).toBe('/')
  expect(body.components?.securitySchemes?.bearer_auth).toBeTruthy()

  const consoleErrors: string[] = []
  const failedUrls: string[] = []
  page.on('console', (msg) => {
    if (msg.type() === 'error') consoleErrors.push(msg.text())
  })
  page.on('pageerror', (err) => consoleErrors.push(String(err)))
  page.on('requestfailed', (req) => {
    failedUrls.push(req.url())
  })

  await page.goto(`${API_URL}/api/docs`, { waitUntil: 'networkidle' })
  await expect
    .poll(async () => page.locator('[class*="scalar"], .scalar-app, #app').count(), {
      timeout: 15_000,
    })
    .toBeGreaterThan(0)

  // Search UI: Scalar shows an "Open Search" control; the searchbox appears after open.
  const openSearch = page.getByRole('button', { name: /Open Search/i })
  await expect(openSearch).toBeVisible({ timeout: 15_000 })
  await openSearch.click()
  await expect(page.getByRole('searchbox', { name: 'Enter search query' })).toBeVisible({
    timeout: 10_000,
  })
  await page.keyboard.press('Escape')

  // Auth + same-origin server (Try it / request playground) on the reference pane.
  await expect(page.getByText(/Authentication/i).first()).toBeVisible({ timeout: 10_000 })
  await expect(page.getByText(/bearer_auth/i).first()).toBeVisible()
  await expect(page.getByText(/same origin/i).first()).toBeVisible()

  // Open a GET operation and confirm Test Request is available for playground calls.
  const health = page.getByRole('link', { name: /\/health/i }).first()
  await health.click()
  await expect(page.getByRole('button', { name: /Test Request/i }).first()).toBeVisible({
    timeout: 10_000,
  })

  const cspOrCdn = consoleErrors.filter((e) =>
    /Content Security Policy|cdn\.jsdelivr|fonts\.scalar|Refused to load/i.test(e),
  )
  expect(cspOrCdn, `CSP/CDN console errors: ${cspOrCdn.join('\\n')}`).toEqual([])

  const remoteFails = failedUrls.filter((u) =>
    /cdn\.jsdelivr|fonts\.scalar|jsdelivr\.net/i.test(u),
  )
  expect(remoteFails, `remote asset failures: ${remoteFails.join('\\n')}`).toEqual([])
})
