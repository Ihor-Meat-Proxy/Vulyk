import { expect, test, type Page } from "@playwright/test";
import { TestIds } from "../../frontend/src/testing/testIds";
import { ensureAdminSession, loginAsAdmin } from "../fixtures/auth";
import { AdminPage } from "../pages/AdminPage";

test.describe.configure({ mode: "serial" });

async function mockAuthStatusOpenMode(page: Page) {
  await page.route("**/api/v1/auth/status", async (route) => {
    if (route.request().method() !== "GET") {
      await route.continue();
      return;
    }
    const response = await route.fetch();
    const body = (await response.json()) as Record<string, unknown>;
    await route.fulfill({
      status: response.status(),
      contentType: "application/json",
      body: JSON.stringify({ ...body, auth_required: false }),
    });
  });
}

test(
  "Admin auth-off banner hidden when auth required; shown when status mocked open",
  {
    tag: ["@VQA-357", "@VUE-219"],
    annotation: [
      { type: "vulyk", description: "VQA-357" },
      { type: "vulyk", description: "VUE-219" },
    ],
  },
  async ({ page, request }) => {
    await ensureAdminSession(request);
    await loginAsAdmin(page);

    const admin = new AdminPage(page);
    await admin.open();
    await expect(page.getByTestId(TestIds.adminPage)).toBeVisible();
    await expect(page.getByTestId(TestIds.adminAuthOffBanner)).toHaveCount(0);

    await mockAuthStatusOpenMode(page);
    await page.reload();
    await expect(page.getByTestId(TestIds.appShell)).toBeVisible({
      timeout: 60_000,
    });
    await admin.open();
    const banner = page.getByTestId(TestIds.adminAuthOffBanner);
    await expect(banner).toBeVisible({ timeout: 20_000 });
    await expect(banner).toContainText(
      /Open mode|відкритий режим|mode ouvert|modo abierto|Offener Modus|authentication off|автентифікація вимкнена|authentification|autenticacion|Authentifizierung/i,
    );
  },
);
