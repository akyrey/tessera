import { defineConfig, devices } from '@playwright/test'
import { E2E_BOOTSTRAP_TOKEN } from './e2e/constants.ts'

const PORT = 8181
const BASE_URL = `http://localhost:${PORT}`

/**
 * Runs against the real Rust server serving the built frontend (`pnpm build` first).
 * The database lives in .e2e/ and is recreated on every run.
 */
export default defineConfig({
  testDir: './e2e',
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  reporter: process.env.CI ? 'github' : 'list',
  use: { baseURL: BASE_URL, trace: 'retain-on-failure' },
  projects: [
    { name: 'android', use: { ...devices['Pixel 7'] } },
    { name: 'iphone', use: { ...devices['iPhone 15'] } },
  ],
  webServer: {
    command: 'rm -rf .e2e && mkdir -p .e2e && cargo run --quiet --manifest-path ../backend/Cargo.toml',
    url: `${BASE_URL}/readyz`,
    reuseExistingServer: false,
    timeout: 300_000,
    env: {
      TESSERA_PUBLIC_URL: BASE_URL,
      TESSERA_LISTEN_ADDR: `127.0.0.1:${PORT}`,
      TESSERA_DATABASE_URL: 'sqlite://.e2e/e2e.db',
      TESSERA_BOOTSTRAP_TOKEN: E2E_BOOTSTRAP_TOKEN,
      // Never reach the real Wikidata/Overpass from tests: nothing listens on port 9.
      TESSERA_WIKIDATA_URL: 'http://127.0.0.1:9/w/api.php',
      TESSERA_OVERPASS_URL: 'http://127.0.0.1:9/api/interpreter',
    },
  },
})
