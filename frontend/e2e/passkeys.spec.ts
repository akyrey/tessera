import { type Page, expect, test } from '@playwright/test'
import { E2E_BOOTSTRAP_TOKEN } from './constants.ts'

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

    await expect(page.getByRole('heading', { name: 'Hi, Alice' })).toBeVisible()
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

  await test.step('a second person joins on their own device', async () => {
    const context = await browser.newContext()
    const memberPage = await context.newPage()
    await addPasskeyDevice(memberPage)

    await memberPage.goto(inviteUrl)
    await expect(memberPage.getByText('invited to join "Home"')).toBeVisible()
    await memberPage.getByLabel('Your name').fill('Bea')
    await memberPage.getByRole('button', { name: 'Create passkey' }).click()
    await expect(memberPage.getByRole('heading', { name: 'Hi, Bea' })).toBeVisible()

    await memberPage.getByRole('link', { name: 'Settings' }).click()
    const household = memberPage.getByRole('region', { name: 'Home' })
    await expect(household.getByText('Alice')).toBeVisible()
    await expect(household.getByText('Bea')).toBeVisible()
    await expect(memberPage.getByRole('button', { name: 'Invite someone' })).toHaveCount(0)
    await context.close()
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
    await expect(page.getByRole('heading', { name: 'Hi, Alice' })).toBeVisible()

    await page.getByRole('link', { name: 'Settings' }).click()
    await expect(page.getByRole('region', { name: 'Passkeys' }).getByText(/last used/)).toBeVisible()
  })
})
