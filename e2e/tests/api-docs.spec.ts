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
  expect(cspBlocked, `CSP/CDN errors: ${cspBlocked.join('\n')}`).toEqual([])
})
