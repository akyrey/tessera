import { type Page, expect, test } from '@playwright/test'
import { E2E_BOOTSTRAP_TOKEN } from './constants.ts'

/** A real 2×2 PNG, so the browser can decode and re-encode it like a camera photo. */
const PHOTO_PNG = Buffer.from(
  'iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAIAAAD91JpzAAAAEElEQVR4nGP4z8AARAwQCgAf7gP9i18U1AAAAABJRU5ErkJggg==',
  'base64',
)

// Chromium's virtual authenticator (CDP) performs real WebAuthn ceremonies
// against the Rust server: real challenges, signatures and verification.
test.skip(({ browserName }) => browserName !== 'chromium', 'virtual authenticators need Chromium (CDP)')

/** A platform authenticator with Face ID-like user verification that always succeeds. */
async function addPasskeyDevice(page: Page) {
  const cdp = await page.context().newCDPSession(page)
  await cdp.send('WebAuthn.enable')
  const { authenticatorId } = await cdp.send('WebAuthn.addVirtualAuthenticator', {
    options: {
      protocol: 'ctap2',
      transport: 'internal',
      hasResidentKey: true,
      hasUserVerification: true,
      isUserVerified: true,
      automaticPresenceSimulation: true,
    },
  })
  return {
    credentials: async () => (await cdp.send('WebAuthn.getCredentials', { authenticatorId })).credentials,
  }
}

test('admin sets up Tessera, invites a household member, and both use passkeys', async ({
  browser,
  page,
}) => {
  const adminDevice = await addPasskeyDevice(page)

  await test.step('admin registers with the bootstrap link', async () => {
    await page.goto(`/join#${E2E_BOOTSTRAP_TOKEN}`)
    await expect(page.getByText('You will be the administrator')).toBeVisible()
    await page.getByLabel('Your name').fill('Alice')
    await page.getByRole('button', { name: 'Create passkey' }).click()

    await expect(page.getByRole('heading', { name: 'Cards', exact: true })).toBeVisible()
    const credentials = await adminDevice.credentials()
    expect(credentials).toHaveLength(1)
    expect(credentials[0]?.isResidentCredential).toBe(true)
  })

  await test.step('the bootstrap link cannot be reused', async () => {
    const reuse = await browser.newPage()
    await reuse.goto(`/join#${E2E_BOOTSTRAP_TOKEN}`)
    await expect(reuse.getByRole('alert')).toHaveText(/invalid, expired or already used/)
    await reuse.close()
  })

  const inviteUrl = await test.step('admin creates a household invite', async () => {
    await page.getByRole('link', { name: 'Settings' }).click()
    await page.getByRole('button', { name: 'Invite someone' }).click()
    const url = await page.getByLabel('Invite link').inputValue()
    // The admin's own bootstrap link no longer lingers in the address bar.
    expect(new URL(page.url()).hash).toBe('')
    expect(url).toMatch(/\/join#[A-Za-z0-9_-]{43}$/)
    return url
  })

  const memberPage = await test.step('a second person joins on their own device', async () => {
    const context = await browser.newContext()
    const memberPage = await context.newPage()
    await addPasskeyDevice(memberPage)

    await memberPage.goto(inviteUrl)
    await expect(memberPage.getByText('invited to join "Home"')).toBeVisible()
    await memberPage.getByLabel('Your name').fill('Bea')
    await memberPage.getByRole('button', { name: 'Create passkey' }).click()
    await expect(memberPage.getByRole('heading', { name: 'Cards', exact: true })).toBeVisible()

    await memberPage.getByRole('link', { name: 'Settings' }).click()
    const household = memberPage.getByRole('region', { name: 'Home' })
    await expect(household.getByText('Alice')).toBeVisible()
    await expect(household.getByText('Bea')).toBeVisible()
    await expect(memberPage.getByRole('button', { name: 'Invite someone' })).toHaveCount(0)
    return memberPage
  })

  await test.step('admin sees the new member', async () => {
    await page.reload()
    await expect(page.getByRole('region', { name: 'Home' }).getByText('Bea')).toBeVisible()
  })

  await test.step('the last passkey cannot be removed', async () => {
    const passkeys = page.getByRole('region', { name: 'Passkeys' })
    await expect(passkeys.getByRole('button', { name: /^Remove / })).toBeDisabled()
  })

  await test.step('admin signs out and back in with the passkey', async () => {
    await page.getByRole('button', { name: 'Sign out', exact: true }).click()
    await expect(page).toHaveURL(/\/login$/)

    await page.getByRole('button', { name: 'Sign in with passkey' }).click()
    await expect(page.getByRole('heading', { name: 'Cards', exact: true })).toBeVisible()

    await page.getByRole('link', { name: 'Settings' }).click()
    await expect(page.getByRole('region', { name: 'Passkeys' }).getByText(/last used/)).toBeVisible()
  })

  await test.step('admin adds a card with a photo and shows it at the till', async () => {
    const consoleErrors: string[] = []
    page.on('console', (message) => {
      if (message.type() === 'error') consoleErrors.push(message.text())
    })
    await page.goto('/')
    await page.getByRole('link', { name: 'Add card' }).click()
    await page.getByLabel('Store name').fill('Esselunga')
    await page.getByLabel('Card number').fill('4006381333932')
    await expect(page.getByText(/last digit does not match/)).toBeVisible()
    await page.getByLabel('Card number').fill('4 006381 333931')
    await page
      .getByLabel('Front photo')
      .setInputFiles({ name: 'front.png', mimeType: 'image/png', buffer: PHOTO_PNG })
    await expect(page.getByRole('img', { name: 'Front' })).toBeVisible()
    await page.getByRole('button', { name: 'Save card' }).click()

    await expect(page.getByRole('heading', { name: 'Esselunga' })).toBeVisible()
    await expect(page.getByRole('img', { name: /EAN-13/ })).toBeVisible()
    await expect(page.getByText('4006381333931')).toBeVisible()
    const photo = page.getByRole('img', { name: 'Front of the card' })
    await expect(photo).toBeVisible()
    await expect.poll(() => photo.evaluate((img: HTMLImageElement) => img.naturalWidth)).toBe(2)

    await page.getByRole('button', { name: 'Favourite' }).click()
    await expect(page.getByRole('button', { name: 'Favourite' })).toHaveAttribute('aria-pressed', 'true')
    await page.getByRole('link', { name: '‹ Cards' }).click()
    await expect(
      page.getByRole('region', { name: 'Favourites' }).getByRole('link', { name: /Esselunga/ }),
    ).toBeVisible()
    expect(consoleErrors).toEqual([])
  })

  await test.step('cards open and can be edited without network, then sync', async () => {
    // The service worker must control the page before the network goes away.
    await page.evaluate(async () => {
      await navigator.serviceWorker.ready
      if (!navigator.serviceWorker.controller) {
        await new Promise((resolve) => navigator.serviceWorker.addEventListener('controllerchange', resolve))
      }
    })
    await page.context().setOffline(true)
    await page.reload()
    await expect(page.getByRole('heading', { name: 'Cards', exact: true })).toBeVisible()
    await page.getByRole('link', { name: /Esselunga/ }).click()
    await expect(page.getByRole('img', { name: /EAN-13/ })).toBeVisible()
    await expect(page.getByRole('img', { name: 'Front of the card' })).toBeVisible()

    await page.getByRole('link', { name: 'Edit' }).click()
    await page.getByLabel('Name on the card (optional)').fill('Alice R.')
    await page.getByRole('button', { name: 'Save card' }).click()
    await expect(page.getByText('Alice R.')).toBeVisible()
    await page.getByRole('link', { name: '‹ Cards' }).click()
    await expect(page.getByText(/\d+ changes? will sync when you are back online/)).toBeVisible()

    await page.context().setOffline(false)
    // Playwright's network emulation does not fire the `online` event that phones do.
    await page.evaluate(() => window.dispatchEvent(new Event('online')))
    await expect(page.getByText(/Offline/)).toBeHidden()
    // A fresh device state proves the edit reached the server.
    await page.evaluate(() => indexedDB.deleteDatabase('tessera'))
    await page.reload()
    await page.getByRole('link', { name: /Esselunga/ }).click()
    await expect(page.getByText('Alice R.')).toBeVisible()
    await page.getByRole('link', { name: '‹ Cards' }).click()
  })

  await test.step('a shop marked by hand brings its card to the top there', async () => {
    await page.context().grantPermissions(['geolocation'])
    await page.context().setGeolocation({ latitude: 45.4642, longitude: 9.19 })
    await page.getByRole('link', { name: /Esselunga/ }).click()
    await page.getByRole('button', { name: /I am at this shop/ }).click()
    await expect(page.getByText(/Esselunga · 45\.4642, 9\.1900/)).toBeVisible()
    await page.getByRole('link', { name: '‹ Cards' }).click()
    await expect(
      page.getByRole('region', { name: 'Nearby' }).getByRole('link', { name: /Esselunga/ }),
    ).toBeVisible()

    // Somewhere else, the card is no longer "nearby".
    await page.context().setGeolocation({ latitude: 41.9, longitude: 12.5 })
    await page.reload()
    await expect(page.getByRole('region', { name: 'Favourites' })).toBeVisible()
    await expect(page.getByRole('region', { name: 'Nearby' })).toHaveCount(0)
  })

  await test.step('admin edits and deletes the card', async () => {
    await page.getByRole('link', { name: /Esselunga/ }).click()
    await page.getByRole('link', { name: 'Edit' }).click()
    await page.getByLabel('Store name').fill('Esselunga Prima')
    await page.getByRole('button', { name: 'Save card' }).click()
    await expect(page.getByRole('heading', { name: 'Esselunga Prima' })).toBeVisible()

    await page.getByRole('link', { name: 'Edit' }).click()
    await page.getByRole('button', { name: 'Delete card' }).click()
    await page.getByRole('button', { name: 'Tap again to delete for good' }).click()
    await expect(page.getByRole('heading', { name: 'No cards yet' })).toBeVisible()
  })

  await test.step('a shared card reaches the household, and unsharing takes it back', async () => {
    await page.getByRole('link', { name: 'Add card' }).click()
    await page.getByLabel('Store name').fill('Coop')
    await page.getByLabel('Barcode type').selectOption('qr')
    await page.getByLabel('Card number').fill('COOP-FAMILY-1')
    await page.getByRole('switch', { name: /Share with Home/ }).check()
    await page.getByRole('button', { name: 'Save card' }).click()
    await expect(page.getByText('Shared with Home')).toBeVisible()

    await memberPage.goto('/')
    const sharedTile = memberPage.getByRole('link', { name: /Coop.*Alice/ })
    await expect(sharedTile).toBeVisible()
    await sharedTile.click()
    await expect(memberPage.getByText('Shared by Alice')).toBeVisible()
    await expect(memberPage.getByRole('img', { name: /QR code/ })).toBeVisible()

    await page.getByRole('link', { name: 'Edit' }).click()
    await page.getByRole('switch', { name: /Share with Home/ }).uncheck()
    await page.getByRole('button', { name: 'Save card' }).click()
    await expect(page.getByText('Shared with Home')).toBeHidden()

    await memberPage.goto('/')
    await expect(memberPage.getByRole('heading', { name: 'No cards yet' })).toBeVisible()
    await memberPage.context().close()
  })
})
