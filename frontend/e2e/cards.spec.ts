import { type Page, type Route, expect, test } from '@playwright/test'

// Runs on both engines. WebKit cannot sign in with a virtual passkey, so the
// JSON API is stubbed here; pages, assets and the CSP still come from the Rust
// server. The full stack is covered by passkeys.spec.ts on Chromium.

const ME = {
  user: { id: 'u-1', displayName: 'Alice', isAdmin: true },
  household: { id: 'h-1', name: 'Home', role: 'owner', members: [] },
}

const card = (id: string, storeName: string, barcodeFormat: string, cardNumber: string) => ({
  id,
  ownerId: 'u-1',
  storeName,
  cardNumber,
  barcodeFormat,
  memberName: null,
  color: null,
  notes: null,
  isFavorite: false,
  useCount: 0,
  lastUsedAt: null,
  frontImageId: null,
  backImageId: null,
  createdAt: 1_800_000_000,
  updatedAt: 1_800_000_000,
})

const CARDS = [
  card('c-ean', 'Esselunga', 'ean13', '4006381333931'),
  card('c-qr', 'Coop', 'qr', 'https://coop.example/c/1'),
  card('c-c128', 'Decathlon', 'code128', 'DK-2026-0042'),
]

const json = (route: Route, body: unknown) => route.fulfill({ json: body })

/** A tiny in-memory server: saved cards show up in the next list. */
async function stubApi(page: Page, uploads: string[] = []) {
  const stored = [...CARDS]
  await page.route('**/api/me', (route) => json(route, ME))
  await page.route('**/api/cards', (route) => json(route, stored))
  await page.route('**/api/cards/*/use', (route) => route.fulfill({ status: 204 }))
  await page.route('**/api/cards/*/images/*', (route) => {
    uploads.push(route.request().headers()['content-type'] ?? '')
    return json(route, { id: 'img-1' })
  })
  await page.route(/\/api\/cards\/[^/]+$/, (route) => {
    const body = route.request().postDataJSON() as Record<string, string>
    const id = new URL(route.request().url()).pathname.split('/').at(-1) ?? ''
    const saved = card(id, body.storeName ?? '', body.barcodeFormat ?? '', body.cardNumber ?? '')
    stored.push(saved)
    return json(route, saved)
  })
}

function collectConsoleErrors(page: Page): string[] {
  const errors: string[] = []
  page.on('console', (message) => {
    if (message.type() === 'error') errors.push(message.text())
  })
  return errors
}

for (const [id, name, label] of [
  ['c-ean', 'Esselunga', /EAN-13/],
  ['c-qr', 'Coop', /QR code/],
  ['c-c128', 'Decathlon', /Code 128/],
] as const) {
  test(`draws the ${name} barcode under the CSP`, async ({ page }) => {
    const errors = collectConsoleErrors(page)
    await stubApi(page)

    await page.goto(`/cards/${id}`)

    const barcode = page.getByRole('img', { name: label })
    await expect(barcode).toBeVisible()
    expect(await barcode.evaluate((img: HTMLImageElement) => img.naturalWidth)).toBeGreaterThan(0)
    expect(errors).toEqual([])
  })
}

test('re-encodes a picked photo in a format the server accepts', async ({ page }) => {
  const uploads: string[] = []
  await stubApi(page, uploads)

  await page.goto('/cards/new')
  await page.getByLabel('Store name').fill('Bennet')
  await page.getByLabel('Barcode type').selectOption('none')
  await page.getByLabel('Card number').fill('123456')
  // A 64×48 photo drawn by the browser itself, as a camera would hand it over.
  const photo = await page.evaluate(async () => {
    const canvas = document.createElement('canvas')
    canvas.width = 64
    canvas.height = 48
    canvas.getContext('2d')?.fillRect(0, 0, 32, 24)
    const blob = await new Promise<Blob | null>((resolve) => canvas.toBlob(resolve, 'image/png'))
    return [...new Uint8Array(await (blob as Blob).arrayBuffer())]
  })
  await page
    .getByLabel('Front photo')
    .setInputFiles({ name: 'card.png', mimeType: 'image/png', buffer: Buffer.from(photo) })
  await expect(page.getByRole('img', { name: 'Front' })).toBeVisible()
  await page.getByRole('button', { name: 'Save card' }).click()

  await expect(page.getByRole('heading', { name: 'Bennet' })).toBeVisible()
  // WebP where the engine can encode it, JPEG otherwise (Safari).
  expect(uploads).toHaveLength(1)
  expect(['image/webp', 'image/jpeg']).toContain(uploads[0])
})
