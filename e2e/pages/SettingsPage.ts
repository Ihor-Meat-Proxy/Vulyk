import { expect, type Page } from '@playwright/test'
import { openUserMenu, userMenuItem } from '../fixtures/header'

const settingsTitle =
  /My settings|Мої налаштування|Mes paramètres|Mis ajustes|Meine Einstellungen/i

const automationHeading =
  /Test automation repositories|Репозиторії тестової автоматизації|Dépôts d.automatisation|Repositorios de automatización|Testautomatisierungs-Repositories/i

const ciTokensHeading =
  /Project CI tokens|CI-токени проєкту|Jetons CI de projet|Tokens CI del proyecto|Projekt-CI-Tokens/i

export class SettingsPage {
  constructor(readonly page: Page) {}

  async open() {
    await openUserMenu(this.page)
    await userMenuItem(this.page, settingsTitle).click()
    await expect(
      this.page.getByRole('heading', { name: settingsTitle }),
    ).toBeVisible({ timeout: 15_000 })
  }

  automationSection() {
    return this.page
      .locator('section.panel')
      .filter({ has: this.page.getByRole('heading', { name: automationHeading }) })
  }

  ciTokensSection() {
    return this.page
      .locator('section.panel')
      .filter({ has: this.page.getByRole('heading', { name: ciTokensHeading }) })
  }

  async expectAutomationHeading() {
    await expect(
      this.page.getByRole('heading', { name: automationHeading }),
    ).toBeVisible({ timeout: 15_000 })
  }

  async expectCiTokensHeading() {
    await expect(
      this.page.getByRole('heading', { name: ciTokensHeading }),
    ).toBeVisible()
  }

  async fillNewRepo(opts: {
    name: string
    remoteUrl: string
    projectKey?: string
  }) {
    const section = this.automationSection()
    await section
      .getByLabel(/^Name$|^Назва$|^Nom$|^Nombre$/i)
      .fill(opts.name)
    await section
      .getByLabel(/Remote URL|URL distante|URL remota|Remote-URL/i)
      .fill(opts.remoteUrl)
    if (opts.projectKey) {
      const box = section
        .locator('fieldset')
        .getByRole('checkbox', { name: new RegExp(opts.projectKey, 'i') })
      if (await box.count()) {
        await box.check()
      }
    }
  }

  async submitAddRepo() {
    await this.automationSection()
      .getByRole('button', {
        name: /Add repository|Додати репозиторій|Ajouter un dépôt|Añadir repositorio|Repository hinzufügen/i,
      })
      .click()
  }

  async selectCiProject(projectId: string, projectKey: string) {
    const select = this.ciTokensSection().getByLabel(
      /Project|Проєкт|Projet|Proyecto|Projekt/i,
    )
    await select.selectOption({ value: projectId }).catch(async () => {
      await select.selectOption({ label: new RegExp(projectKey) })
    })
  }

  async createCiToken(name: string) {
    const section = this.ciTokensSection()
    await section
      .getByLabel(/Token name|Назва токена|Nom du jeton|Nombre del token|Tokenname/i)
      .fill(name)
    await section
      .getByRole('button', {
        name: /Create CI token|Створити CI-токен|Créer un jeton CI|Crear token CI|CI-Token erstellen/i,
      })
      .click()
  }
}
