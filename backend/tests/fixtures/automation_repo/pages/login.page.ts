export class LoginPage {
  constructor(readonly page: import('@playwright/test').Page) {}
  async goto() {
    await this.page.goto('/login');
  }
  async fillCredentials(email: string, password: string) {
    await this.page.getByLabel(/email/i).fill(email);
    await this.page.getByLabel(/password/i).fill(password);
  }
}
