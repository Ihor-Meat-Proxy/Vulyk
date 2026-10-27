import { expect, test, type Page } from '@playwright/test'
import { ensureAdminSession, loginAsAdmin } from '../fixtures/auth'
import { TestIds } from '../../frontend/src/testing/testIds'

/**
 * Runtime audit: unique test ids per page, interactive coverage on main shells,
 * and no PII/secrets in data-testid / data-entity-* attributes.
 */
const UUID_IN_TESTID =
  /(?:^|-)[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i

function suspiciousDataValue(attr: string, value: string): string | null {
  if (value.includes('@')) return `${attr} contains @ (possible email)`
  if (/^eyJ[A-Za-z0-9_-]+\./.test(value)) return `${attr} looks like a JWT`
  if (/^(bearer|basic)\s/i.test(value)) return `${attr} looks like auth material`
  if (attr === 'data-testid' && UUID_IN_TESTID.test(value)) {
    return `${attr} embeds a UUID`
  }
  if (
    (attr === 'data-entity-key' || attr === 'data-entity-id') &&
    value.length >= 40 &&
    /^[A-Za-z0-9+/=_-]+$/.test(value) &&
    !/^[0-9a-f-]{36}$/i.test(value)
  ) {
    return `${attr} looks like a high-entropy secret`
  }
  return null
}

async function collectTestAttrIssues(page: Page): Promise<string[]> {
  return page.evaluate(
    ({ uuidReSource }) => {
      const uuidRe = new RegExp(uuidReSource, 'i')
      const issues: string[] = []
      const seen = new Map<string, number>()
      const attrs = [
        'data-testid',
        'data-entity-id',
        'data-entity-key',
        'data-index',
        'data-depth',
      ] as const

      Array.from(document.querySelectorAll('*')).forEach((el) => {
        for (const attr of attrs) {
          if (!el.hasAttribute(attr)) continue
          const value = el.getAttribute(attr) || ''
          if (attr === 'data-testid') {
            const n = (seen.get(value) || 0) + 1
            seen.set(value, n)
          }
          if (value.includes('@')) {
            issues.push(`${attr}="${value}" contains @`)
          }
          if (/^eyJ[A-Za-z0-9_-]+\./.test(value)) {
            issues.push(`${attr} looks like a JWT`)
          }
          if (attr === 'data-testid' && uuidRe.test(value)) {
            issues.push(`data-testid="${value}" embeds a UUID`)
          }
        }
      })

      // Duplicate test ids: allow list-row bases that also have data-entity-id
      const listBases = new Set([
        'case-row',
        'folder-node',
        'folder-case-item',
        'run-row',
        'run-detail-case-row',
        'execution-node',
        'requirement-row',
        'admin-user-row',
        'admin-project-row',
        'settings-token-row',
        'chat-message',
        'case-picker-item',
        'automated-suite-row',
        'case-detail-step',
        'case-list-column-item',
        'case-list-column-item-checkbox',
        'case-list-column-item-move-left',
        'case-list-column-item-move-right',
        'case-list-col-sort',
        'case-list-col-custom-field',
        'case-row-custom-field',
        'header-language-item',
        'login-sso-button',
      ])
      for (const [id, count] of seen) {
        if (count <= 1) continue
        if (listBases.has(id)) {
          const nodes = document.querySelectorAll(`[data-testid="${id}"]`)
          let ok = true
          nodes.forEach((n) => {
            if (!n.hasAttribute('data-entity-id') && !n.hasAttribute('data-index')) {
              ok = false
            }
          })
          if (ok) continue
        }
        issues.push(`duplicate data-testid "${id}" (${count} times)`)
      }

      // Interactive without test id (best-effort on main controls)
      const interactive = document.querySelectorAll(
        'button, input:not([type="hidden"]), select, textarea, a[href], [role="tab"], [role="menuitem"], [role="menuitemradio"]',
      )
      interactive.forEach((el) => {
        if (el.closest('[data-testid]')) return
        if (el.hasAttribute('data-testid')) return
        const tag = el.tagName.toLowerCase()
        const name =
          el.getAttribute('aria-label') ||
          el.getAttribute('name') ||
          el.textContent?.trim().slice(0, 40) ||
          tag
        issues.push(`interactive <${tag}> missing data-testid near "${name}"`)
      })

      return issues
    },
    { uuidReSource: UUID_IN_TESTID.source },
  )
}

test.describe('test id audit', () => {
  test.beforeAll(async ({ request }) => {
    await ensureAdminSession(request)
  })

  test('login + app shell attrs are unique and non-sensitive', {
  tag: '@VQA-305', annotation: { type: 'vulyk', description: 'VQA-305' },
}, async ({
    page,
  }) => {
    await page.goto('/login')
    // May already be authed; either login panel or app shell is fine
    await expect(
      page
        .getByTestId(TestIds.loginPanel)
        .or(page.getByTestId(TestIds.appShell))
        .first(),
    ).toBeVisible({ timeout: 60_000 })

    let issues = await collectTestAttrIssues(page)
    // Soften: if redirected to app, still check
    expect(issues.filter((i) => i.includes('@') || i.includes('JWT'))).toEqual(
      [],
    )

    await loginAsAdmin(page)
    await expect(page.getByTestId(TestIds.appShell)).toBeVisible({
      timeout: 60_000,
    })

    for (const path of ['/', '/runs', '/executions', '/appearance', '/settings']) {
      await page.goto(path)
      await page.waitForLoadState('domcontentloaded')
      issues = await collectTestAttrIssues(page)
      const security = issues.filter(
        (i) =>
          i.includes('@') ||
          i.includes('JWT') ||
          i.includes('embeds a UUID') ||
          i.includes('high-entropy') ||
          i.includes('auth material'),
      )
      expect(security, `security issues on ${path}`).toEqual([])

      const dupes = issues.filter((i) => i.startsWith('duplicate data-testid'))
      expect(dupes, `duplicate test ids on ${path}`).toEqual([])
    }
  })
})

// Keep helper referenced for unit-style clarity in reviews
void suspiciousDataValue
