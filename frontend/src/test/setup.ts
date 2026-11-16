import 'fake-indexeddb/auto'
import '@testing-library/jest-dom/vitest'
import { cleanup, configure } from '@testing-library/react'
import { afterEach, beforeEach } from 'vitest'
import { clearLocalData } from '../offline/db'

// IndexedDB round-trips make some screens settle a little later under a parallel run.
configure({ asyncUtilTimeout: 3_000 })

// Vitest globals are off, so Testing Library cannot register this itself.
afterEach(cleanup)

// Each test starts with an empty device.
beforeEach(async () => {
  // Imported lazily so each test file's `vi.mock` applies to the engine's API calls.
  const { resetSyncForTests } = await import('../offline/syncEngine')
  resetSyncForTests()
  await clearLocalData()
})
