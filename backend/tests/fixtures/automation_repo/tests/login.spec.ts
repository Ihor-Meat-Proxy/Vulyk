import { test, expect } from '@playwright/test';
import { LoginPage } from '../pages/login.page';

test('user can sign in', async ({ page }) => {
  const login = new LoginPage(page);
  await login.goto();
  await login.fillCredentials('admin@example.com', 'secret');
  await expect(page.getByRole('heading')).toBeVisible();
});
