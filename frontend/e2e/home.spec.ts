import { expect, test } from '@playwright/test'

test('signed-out visitors land on sign-in, served by the Rust server', async ({ page }) => {
  const consoleErrors: string[] = []
  page.on('console', (message) => {
    if (message.type() === 'error') consoleErrors.push(message.text())
  })

  await page.goto('/')

  await expect(page).toHaveURL(/\/login$/)
  await expect(page.getByRole('heading', { name: 'Tessera' })).toBeVisible()
  await expect(page.getByRole('button', { name: 'Sign in with passkey' })).toBeVisible()
  await expect(page.getByRole('status')).toHaveText(/Server online · v\d+\.\d+\.\d+/)
  // A CSP violation or a failed asset shows up as a console error; the 401
  // from the session check is expected and filtered out.
  expect(consoleErrors.filter((text) => !text.includes('401'))).toEqual([])
})

test('deep links are served by the SPA fallback', async ({ page }) => {
  const response = await page.goto('/settings')

  expect(response?.status()).toBe(200)
  await expect(page).toHaveURL(/\/login$/)
})

test('an invalid invite link is explained', async ({ page }) => {
  await page.goto('/join#not-a-real-token')

  await expect(page.getByRole('alert')).toHaveText(/invalid, expired or already used/)
})

test('the app is installable: manifest and service worker are served', async ({ page, request }) => {
  await page.goto('/login')

  const manifestHref = await page.locator('link[rel="manifest"]').getAttribute('href')
  expect(manifestHref).toBeTruthy()
  const manifest = await (await request.get(manifestHref!)).json()
  expect(manifest).toMatchObject({ name: 'Tessera', display: 'standalone' })

  const registration = await page.evaluate(
    async () => (await navigator.serviceWorker.ready).active?.scriptURL,
  )
  expect(registration).toMatch(/\/sw\.js$/)
})

test('the app shell opens without network once installed', async ({ page, context }) => {
  await page.goto('/login')
  await page.evaluate(async () => {
    await navigator.serviceWorker.ready
  })
  // Precaching finishes during install; a broken precache list fails here.
  await expect
    .poll(() => page.evaluate(async () => (await caches.keys()).some((name) => name.includes('precache'))))
    .toBe(true)

  await context.setOffline(true)
  await page.reload()

  await expect(page.getByRole('heading', { name: 'Tessera' })).toBeVisible()
})
