import { defineConfig, devices } from '@playwright/test';

/**
 * OpenKite E2E + visual regression config.
 *
 * Targets the web build (`dx serve --web`) — desktop and web share the same
 * RSX/CSS, so web is the cheapest headless proxy. CI runs the webServer
 * itself; local runs reuse an already-running `dx serve --web`.
 */
export default defineConfig({
  testDir: './tests',
  fullyParallel: true,
  retries: process.env.CI ? 2 : 0,
  reporter: [['html', { open: 'never' }], ['github']],
  timeout: 30_000,
  use: {
    baseURL: 'http://localhost:8080',
    trace: 'on-first-retry',
    screenshot: 'only-on-failure',
  },
  projects: [
    { name: 'chromium', use: { ...devices['Desktop Chrome'] } },
  ],
  webServer: {
    command: 'dx serve --web --port 8080',
    port: 8080,
    timeout: 10 * 60 * 1000,
    reuseExistingServer: !process.env.CI,
    env: {
      // Headless web build: no desktop/webview deps needed.
      DIOXUS_WEB_SERVE: '1',
    },
  },
});
