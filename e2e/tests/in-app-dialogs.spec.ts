import { expect, test } from "@playwright/test";
import { TestIds } from "../../frontend/src/testing/testIds";
import { createProject } from "../fixtures/api";
import { ensureAdminSession, loginAsAdmin } from "../fixtures/auth";
import { uniqueKey } from "../fixtures/env";
import { CasesPage } from "../pages/CasesPage";
import { ExecutionsPage } from "../pages/ExecutionsPage";

test.describe.configure({ mode: "serial" });

async function selectProject(
  page: import("@playwright/test").Page,
  key: string,
) {
  const select = page.getByTestId(TestIds.headerProjectSelect);
  await expect
    .poll(async () => select.locator("option").count())
    .toBeGreaterThan(0);
  const options = await select.locator("option").allTextContents();
  const idx = options.findIndex((o) => o.includes(key));
  if (idx >= 0) await select.selectOption({ index: idx });
}

test("campaign dialog creates campaign from Executions", async ({
  page,
  request,
}) => {
  const admin = await ensureAdminSession(request);
  const key = uniqueKey("CD");
  await createProject(request, admin.token, {
    key,
    name: `Campaign dialog ${key}`,
  });
  const campaignName = `Campaign ${key}`;

  await loginAsAdmin(page);
  await selectProject(page, key);

  const executions = new ExecutionsPage(page);
  await executions.open();
  await executions.createCampaign(campaignName);

  await expect(
    page
      .getByTestId(TestIds.executionsTree)
      .getByTestId(TestIds.executionNode)
      .filter({ hasText: campaignName }),
  ).toBeVisible({ timeout: 20_000 });
});

test("folder delete dialog shows counts and moves cases to parent", async ({
  page,
  request,
}) => {
  const admin = await ensureAdminSession(request);
  const key = uniqueKey("FD");
  await createProject(request, admin.token, {
    key,
    name: `Folder delete ${key}`,
  });
  const folderName = `Folder ${key}`;
  const caseTitle = `Case in folder ${key}`;

  await loginAsAdmin(page);
  await selectProject(page, key);

  const cases = new CasesPage(page);
  await cases.open();
  await expect(cases.folderTree()).toBeVisible({ timeout: 20_000 });
  await cases.createFolder(folderName);
  await expect(cases.folderTree().getByText(folderName)).toBeVisible({
    timeout: 15_000,
  });

  await cases.selectFolder(folderName);
  await cases.createCase(caseTitle);

  await cases.deleteFolder(folderName);

  const dialog = cases.folderDeleteDialog();
  await expect(dialog).toBeVisible({ timeout: 10_000 });
  await expect(
    dialog.getByTestId(TestIds.folderDeleteDialogMoveOption),
  ).toBeVisible();
  await expect(
    dialog.getByTestId(TestIds.folderDeleteDialogDeleteCasesOption),
  ).toBeVisible();

  const moveLabel = dialog.getByTestId(TestIds.folderDeleteDialogMoveLabel);
  await expect(moveLabel).toBeVisible();
  await expect(moveLabel).toContainText(/1/);

  const deleteLabel = dialog.getByTestId(TestIds.folderDeleteDialogDeleteLabel);
  await expect(deleteLabel).toContainText(/1/);

  await cases.confirmFolderDeleteMoveToParent();

  await expect(cases.folderTree().getByText(folderName)).toHaveCount(0, {
    timeout: 20_000,
  });
  await expect(cases.listPanel().getByText(caseTitle)).toBeVisible({
    timeout: 20_000,
  });
});
