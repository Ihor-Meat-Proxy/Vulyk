import { expect, type Page } from '@playwright/test'
import { TestIds } from '../../frontend/src/testing/testIds'
import { openUserMenu } from '../fixtures/header'

export class AdminPage {
  constructor(readonly page: Page) {}

  async open() {
    await openUserMenu(this.page)
    await this.page.getByTestId(TestIds.headerGearAdmin).click()
    await expect(this.page.getByTestId(TestIds.adminPage)).toBeVisible()
  }

  async openTab(
    tab:
      | 'users'
      | 'roles'
      | 'projects'
      | 'tokens'
      | 'llm'
      | 'integrations'
      | 'sso'
      | 'settings'
      | 'lists'
      | 'customFields'
      | 'config'
      | RegExp
      | string,
  ) {
    const map: Record<string, string> = {
      users: TestIds.adminTabUsers,
      roles: TestIds.adminTabRoles,
      projects: TestIds.adminTabProjects,
      tokens: TestIds.adminTabTokens,
      llm: TestIds.adminTabLlm,
      integrations: TestIds.adminTabIntegrations,
      sso: TestIds.adminTabSso,
      settings: TestIds.adminTabSettings,
      lists: TestIds.adminTabLists,
      customFields: TestIds.adminTabCustomFields,
      config: TestIds.adminTabConfig,
    }
    if (typeof tab === 'string' && map[tab]) {
      await this.page.getByTestId(map[tab]).click()
      return
    }
    // Legacy: role name / locale regex used by older specs
    await this.page.getByRole('tab', { name: tab }).click()
  }

  async createUser(email: string, displayName: string, password: string) {
    await this.openTab('users')
    await this.page.getByTestId(TestIds.adminUserEmailInput).fill(email)
    await this.page
      .getByTestId(TestIds.adminUserDisplayNameInput)
      .fill(displayName)
    await this.page.getByTestId(TestIds.adminUserPasswordInput).fill(password)
    await this.page.getByTestId(TestIds.adminUserCreateButton).click()
    await expect(this.page.getByText(email)).toBeVisible({ timeout: 20_000 })
  }

  async assignRole(userEmail: string, roleLabel: string) {
    await this.openTab('roles')
    await this.page
      .getByTestId(TestIds.adminRoleUserSelect)
      .selectOption({ label: userEmail })
    await this.page
      .getByTestId(TestIds.adminRoleSelect)
      .selectOption({ label: roleLabel })
    await this.page.getByTestId(TestIds.adminRoleSaveButton).click()
    await expect(this.page.getByText(userEmail)).toBeVisible()
  }

  async createProject(name: string, key?: string) {
    await this.openTab('projects')
    await this.page.getByTestId(TestIds.adminProjectNameInput).fill(name)
    if (key) {
      await this.page.getByTestId(TestIds.adminProjectKeyInput).fill(key)
    }
    await this.page.getByTestId(TestIds.adminProjectCreateButton).click()
    await expect(
      this.page
        .getByTestId(TestIds.adminProjectRow)
        .filter({ hasText: name })
        .first(),
    ).toBeVisible({ timeout: 20_000 })
  }

  async openLlmTab() {
    await this.openTab('llm')
  }

  async openIntegrationsTab() {
    await this.openTab('integrations')
  }

  async openSettingsTab() {
    await this.openTab('settings')
  }

  async openSsoTab() {
    await this.openTab('sso')
  }

  async openTokensTab() {
    await this.openTab('tokens')
  }
}
