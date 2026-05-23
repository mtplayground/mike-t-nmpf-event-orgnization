import { defineConfig, devices } from '@playwright/test';

const appPort = Number(process.env.PLAYWRIGHT_APP_PORT ?? 4173);
const apiPort = Number(process.env.PLAYWRIGHT_API_PORT ?? 4174);

export default defineConfig({
  testDir: './e2e',
  fullyParallel: false,
  reporter: process.env.CI ? 'github' : 'list',
  use: {
    baseURL: `http://127.0.0.1:${appPort}`,
    trace: 'on-first-retry',
  },
  webServer: {
    command: `npm run dev -- --host 127.0.0.1 --port ${appPort}`,
    env: {
      VITE_API_BASE_URL: `http://127.0.0.1:${apiPort}`,
    },
    reuseExistingServer: !process.env.CI,
    timeout: 120_000,
    url: `http://127.0.0.1:${appPort}`,
  },
  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
  ],
});
