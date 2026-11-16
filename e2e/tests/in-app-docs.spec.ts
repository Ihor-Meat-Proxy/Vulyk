import { expect, test } from '@playwright/test'
import { TestIds } from '../../frontend/src/testing/testIds'
import { ensureAdminSession, loginAsAdmin } from '../fixtures/auth'
import { API_URL } from '../fixtures/env'
import { openUserMenu } from '../fixtures/header'

test('gear Docs opens in-app help; TOC navigates; API docs opens Scalar', {
  tag: ['@VUE-235'],
  annotation: [{ type: 'vulyk', description: 'VUE-235' }],
}, async ({ page, request }) => {
  await ensureAdminSession(request)
  await loginAsAdmin(page)
  await expect(page.getByTestId(TestIds.appShell)).toBeVisible({
    timeout: 60_000,
  })

  await openUserMenu(page)
  await expect(page.getByTestId(TestIds.docsMenuLink)).toBeVisible()
  await expect(page.getByTestId(TestIds.apiDocsMenuLink)).toBeVisible()

  await page.getByTestId(TestIds.docsMenuLink).click()
  await expect(page.getByTestId(TestIds.docsPage)).toBeVisible({
    timeout: 20_000,
  })
  await expect(page).toHaveURL(/\/docs(\/|$)/)
  await expect(page.getByTestId(TestIds.docsSidebar)).toBeVisible()
  await expect(page.getByTestId(TestIds.docsArticle)).toBeVisible()

  const toc = page.getByTestId(TestIds.docsToc)
  await expect(toc).toBeVisible()
  const concepts = page.getByTestId(`${TestIds.docsTocPagePrefix}concepts`)
  if (await concepts.count()) {
    await concepts.click()
    await expect(page).toHaveURL(/\/docs\/concepts/)
    await expect(page.getByTestId(TestIds.docsArticle)).toContainText(
      /Concepts|concept/i,
    )
  } else {
    // Shell-only deploy may only ship overview until content PR lands.
    await expect(
      page.getByTestId(`${TestIds.docsTocPagePrefix}overview`),
    ).toBeVisible()
  }

  const headingLink = page.getByTestId(TestIds.docsTocHeading).first()
  if (await headingLink.count()) {
    const href = await headingLink.getAttribute('href')
    expect(href).toMatch(/^#/)
    await headingLink.click()
  }

  await openUserMenu(page)
  const apiDocs = page.getByTestId(TestIds.apiDocsMenuLink)
  await expect(apiDocs).toHaveAttribute('href', /\/api\/docs/)
  const [popup] = await Promise.all([
    page.waitForEvent('popup'),
    apiDocs.click(),
  ])
  await popup.waitForLoadState('domcontentloaded')
  await expect(popup.locator('#api-reference')).toBeAttached({
    timeout: 20_000,
  })
})

test('OpenAPI includes public CI ingest endpoints used in docs examples', {
  tag: ['@VUE-236'],
  annotation: [{ type: 'vulyk', description: 'VUE-236' }],
}, async ({ request }) => {
  const spec = await request.get(`${API_URL}/api/openapi.json`)
  expect(spec.ok()).toBeTruthy()
  const body = (await spec.json()) as { paths?: Record<string, unknown> }
  const paths = Object.keys(body.paths ?? {})

  const required = [
    '/api/v1/projects/{project_id}/ci/runs',
    '/api/v1/projects/{project_id}/ci/runs/{run_id}/ingest',
    '/api/v1/projects/{project_id}/ci/runs/{run_id}/close',
    '/api/v1/projects/{project_id}/ingest',
  ]
  for (const p of required) {
    expect(paths, `missing OpenAPI path ${p}`).toContain(p)
  }
})
