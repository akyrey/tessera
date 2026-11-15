import { expect, test } from '@playwright/test'

test('home page loads from the Rust server and reports it online', async ({ page }) => {
  const consoleErrors: string[] = []
  page.on('console', (message) => {
    if (message.type() === 'error') consoleErrors.push(message.text())
  })

  await page.goto('/')

  await expect(page.getByRole('heading', { name: 'Tessera' })).toBeVisible()
  await expect(page.getByRole('status')).toHaveText(/Server online · v\d+\.\d+\.\d+/)
  // A CSP violation or a failed asset shows up as a console error.
  expect(consoleErrors).toEqual([])
})

test('deep links are served by the SPA fallback', async ({ page }) => {
  const response = await page.goto('/cards/some-id')

  expect(response?.status()).toBe(200)
  await expect(page.getByRole('heading', { name: 'Tessera' })).toBeVisible()
})

test('the app is installable: manifest and service worker are served', async ({ page, request }) => {
  await page.goto('/')

  const manifestHref = await page.locator('link[rel="manifest"]').getAttribute('href')
  expect(manifestHref).toBeTruthy()
  const manifest = await (await request.get(manifestHref!)).json()
  expect(manifest).toMatchObject({ name: 'Tessera', display: 'standalone' })

  const registration = await page.evaluate(
    async () => (await navigator.serviceWorker.ready).active?.scriptURL,
  )
  expect(registration).toMatch(/\/sw\.js$/)
})
