import { randomBytes } from 'node:crypto'

const TOKEN_BYTES = 32

// A fresh, strong first-admin token per run. Set on the environment when the
// runner loads the config, so the server and every worker see the same value.
process.env.E2E_BOOTSTRAP_TOKEN ??= randomBytes(TOKEN_BYTES).toString('base64url')

export const E2E_BOOTSTRAP_TOKEN: string = process.env.E2E_BOOTSTRAP_TOKEN
