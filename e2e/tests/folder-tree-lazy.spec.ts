import { expect, test } from '@playwright/test'
import { TestIds } from '../../frontend/src/testing/testIds'
import {
  apiJson,
  createCase,
  createProject,
} from '../fixtures/api'
import { ensureAdminSession, loginAsAdmin } from '../fixtures/auth'
import { uniqueKey } from '../fixtures/env'
import { CasesPage } from '../pages/CasesPage'

/**
 * Covers the lazy folder-tree fix: never load the whole project into the tree
 * with a global capped fetch. Shape mirrors the 10×10×200 perf project at a
 * smaller scale that still needs pagination (220 direct cases → Load more).
 */
test.describe('folder tree lazy load + numeric sort', () => {
  test('recursive counts, expand loads 100, numeric key order', {
  tag: '@VQA-254', annotation: { type: 'vulyk', description: 'VQA-254' },
}, async ({
    page,
    request,
  }) => {
    const admin = await ensureAdminSession(request)
    const key = uniqueKey('FT')
    const project = await createProject(request, admin.token, {
      key,
      name: `Folder Tree ${key}`,
    })

    const parent = await apiJson<{ id: string; name: string }>(
      request,
      'POST',
      `/api/v1/projects/${project.id}/suites`,
      { token: admin.token, data: { name: 'Area-0', sort_order: 0 } },
    )
    const leaf = await apiJson<{ id: string; name: string }>(
      request,
      'POST',
      `/api/v1/projects/${project.id}/suites`,
      {
        token: admin.token,
        data: { name: 'Leaf-0-0', parent_id: parent.id, sort_order: 0 },
      },
    )

    // 220 leaf cases (Load more at 100) + 1 parent-direct → recursive 221.
    // Batched creates keep the seed under a minute while mirroring the 10×10×200 shape.
    const body = (title: string, suiteId: string) => ({
      title,
      suite_id: suiteId,
      status: 'ready',
      priority: 'medium',
      steps: [] as unknown[],
    })
    for (let start = 1; start <= 220; start += 20) {
      const end = Math.min(start + 19, 220)
      await Promise.all(
        Array.from({ length: end - start + 1 }, (_, k) => {
          const i = start + k
          return createCase(
            request,
            admin.token,
            project.id,
            body(`Leaf case ${i}`, leaf.id),
          )
        }),
      )
    }
    await createCase(
      request,
      admin.token,
      project.id,
      body('Parent direct', parent.id),
    )

    const suites = await apiJson<
      {
        id: string
        name: string
        case_count: number
        direct_case_count: number
      }[]
    >(request, 'GET', `/api/v1/projects/${project.id}/suites`, {
      token: admin.token,
    })
    const parentSuite = suites.find((s) => s.id === parent.id)!
    const leafSuite = suites.find((s) => s.id === leaf.id)!
    expect(parentSuite.case_count).toBe(221)
    expect(parentSuite.direct_case_count).toBe(1)
    expect(leafSuite.case_count).toBe(220)
    expect(leafSuite.direct_case_count).toBe(220)

    // List API numeric order (string sort would put -10 before -2).
    const listed = await apiJson<{ items: { key: string }[]; total: number }>(
      request,
      'GET',
      `/api/v1/projects/${project.id}/cases?suite_id=${leaf.id}&direct_only=true&limit=20`,
      { token: admin.token },
    )
    expect(listed.total).toBe(220)
    const keys = listed.items.map((c) => c.key)
    const prefix = keys[0].split('-')[0]
    expect(keys.indexOf(`${prefix}-2`)).toBeLessThan(
      keys.indexOf(`${prefix}-10`),
    )

    await loginAsAdmin(page)
    const select = page.getByTestId(TestIds.headerProjectSelect)
    await expect
      .poll(async () => select.locator('option').count())
      .toBeGreaterThan(0)
    const options = await select.locator('option').allTextContents()
    const idx = options.findIndex((o) => o.includes(key))
    expect(idx).toBeGreaterThanOrEqual(0)
    await select.selectOption({ index: idx })
    const cases = new CasesPage(page)
    await cases.open()
    await expect(cases.folderTree()).toBeVisible({ timeout: 20_000 })

    // Recursive badge on parent (visible at root); leaf only appears after expand.
    const parentCount = cases
      .folderTree()
      .locator(
        `[data-testid="folder-node"][data-entity-id="${parent.id}"] [data-testid="folder-node-count"]`,
      )
    await expect(parentCount).toHaveText('221', { timeout: 20_000 })
    await expect(parentCount).toHaveAttribute('title', /221 \(1 direct\)/)

    await cases
      .folderTree()
      .locator('.folder-tree-folder')
      .filter({ hasText: 'Area-0' })
      .getByRole('button', { name: /Expand|Розгорнути/i })
      .click()

    const leafCount = cases
      .folderTree()
      .locator(
        `[data-testid="folder-node"][data-entity-id="${leaf.id}"] [data-testid="folder-node-count"]`,
      )
    await expect(leafCount).toHaveText('220', { timeout: 20_000 })

    // Expand leaf — cases appear only after expand (lazy, 100/page).
    await cases
      .folderTree()
      .locator('.folder-tree-folder')
      .filter({ hasText: 'Leaf-0-0' })
      .getByRole('button', { name: /Expand|Розгорнути/i })
      .click()

    // Parent expand also lazy-loads its 1 direct case, so total visible = 100 leaf + 1 parent.
    const treeCases = cases.folderTree().locator('.folder-tree-case')
    await expect(treeCases.first()).toBeVisible({ timeout: 20_000 })
    await expect
      .poll(async () => treeCases.count())
      .toBe(101)

    const firstKeys = await treeCases
      .locator('.folder-tree-case-key')
      .allTextContents()
    expect(firstKeys[0]?.trim()).toBe(`${prefix}-1`)
    expect(firstKeys.indexOf(`${prefix}-2`)).toBeLessThan(
      firstKeys.indexOf(`${prefix}-10`),
    )

    await cases
      .folderTree()
      .getByRole('button', { name: /Load more|Завантажити ще/i })
      .click()
    await expect
      .poll(async () => treeCases.count())
      .toBe(201)

    // Selecting the leaf filters the right-hand list to 220 (not a capped handful).
    await cases.selectFolder(/Leaf-0-0/)
    await expect(cases.caseCount()).toContainText(
      '220',
      { timeout: 20_000 },
    )
  })
})
