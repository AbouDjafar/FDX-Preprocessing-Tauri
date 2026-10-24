import { defineConfig, devices } from '@playwright/test';

const displayProjects = [
  { width: 1024, height: 768 },
  { width: 1366, height: 768 },
  { width: 1920, height: 1080 },
].flatMap(viewport => [1.25, 1.5, 2].map(deviceScaleFactor => ({
  name: `layout-${viewport.width}x${viewport.height}-dpr-${deviceScaleFactor}`,
  testMatch: '**/layout.spec.ts',
  use: { ...devices['Desktop Chrome'], viewport, deviceScaleFactor, reducedMotion: 'reduce' as const },
})));

export default defineConfig({
  testDir: './tests/e2e',
  outputDir: './test-results',
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  workers: process.env.CI ? 2 : undefined,
  timeout: 45000,
  expect: { timeout: 10000 },
  reporter: [['list'], ['html', { outputFolder: 'test-results/report', open: 'never' }]],
  use: { baseURL: 'http://127.0.0.1:4173', trace: 'retain-on-failure', screenshot: 'on', video: 'retain-on-failure', acceptDownloads: true },
  projects: [
    { name: 'desktop', testIgnore: '**/layout.spec.ts', use: { ...devices['Desktop Chrome'], viewport: { width: 1440, height: 1000 } } },
    { name: 'mobile', testIgnore: '**/layout.spec.ts', use: { ...devices['iPhone 13'], defaultBrowserType: 'chromium', reducedMotion: 'reduce' } },
    ...displayProjects,
    {
      name: 'layout-css-zoom-200', testMatch: '**/layout.spec.ts', metadata: { cssZoom: 2 },
      use: { ...devices['Desktop Chrome'], viewport: { width: 1366, height: 768 }, deviceScaleFactor: 1, reducedMotion: 'reduce' },
    },
    {
      name: 'layout-logical-640x480', testMatch: '**/layout.spec.ts',
      use: { ...devices['Desktop Chrome'], viewport: { width: 640, height: 480 }, deviceScaleFactor: 1.5, reducedMotion: 'reduce' },
    },
  ],
  webServer: { command: 'npm run preview -- --port 4173 --strictPort', url: 'http://127.0.0.1:4173', reuseExistingServer: !process.env.CI, timeout: 30000 },
});
