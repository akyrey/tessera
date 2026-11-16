import { describe, expect, it, vi } from 'vitest'
import { card } from '../../test/fixtures'
import { db } from '../../offline/db'
import { BACKUP_FORMAT, type Backup } from './backupFormat'
import { buildBackup } from './exportCards'
import { BackupError, importBackup, readBackup } from './importCards'

vi.mock('../../api/cards', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../../api/cards')>()),
}))

const PNG = 'data:image/png;base64,iVBORw0KGgo='
const file = (content: unknown) => new Blob([typeof content === 'string' ? content : JSON.stringify(content)])

const backupOf = (...cards: Backup['cards']): Backup => ({
  format: BACKUP_FORMAT,
  version: 1,
  exportedAt: '2027-01-01T00:00:00.000Z',
  cards,
})

const entry = (overrides: Partial<Backup['cards'][number]> = {}): Backup['cards'][number] => ({
  storeName: 'Coop',
  cardNumber: '4006381333931',
  barcodeFormat: 'ean13',
  memberName: null,
  color: null,
  notes: null,
  expiresOn: null,
  isFavorite: false,
  brandName: null,
  places: [],
  photos: {},
  ...overrides,
})

describe('export', () => {
  it('embeds server photos and local previews as data URLs', async () => {
    vi.stubGlobal(
      'fetch',
      vi
        .fn()
        .mockResolvedValue(new Response('png', { status: 200, headers: { 'Content-Type': 'image/png' } })),
    )
    const cards = [
      card({ frontImageId: 'img-1', isFavorite: true }),
      card({ id: 'c2', localPhotos: { back: PNG } } as Parameters<typeof card>[0]),
    ]

    const backup = await buildBackup(cards, new Date('2027-01-01T00:00:00Z'))

    expect(backup.exportedAt).toBe('2027-01-01T00:00:00.000Z')
    expect(backup.cards[0]?.photos.front).toMatch(/^data:image\/png;base64,/)
    expect(backup.cards[0]?.isFavorite).toBe(true)
    expect(backup.cards[1]?.photos).toEqual({ back: PNG })
    // Round trip: what we export, we can read back.
    await expect(readBackup(file(backup))).resolves.toEqual(backup)
  })

  it('leaves out photos that cannot be fetched or are not images', async () => {
    vi.stubGlobal('fetch', vi.fn().mockRejectedValue(new TypeError('offline')))
    expect((await buildBackup([card({ frontImageId: 'img-1' })])).cards[0]?.photos).toEqual({})

    vi.stubGlobal(
      'fetch',
      vi.fn().mockResolvedValue(new Response('<html>', { headers: { 'Content-Type': 'text/html' } })),
    )
    expect((await buildBackup([card({ frontImageId: 'img-1' })])).cards[0]?.photos).toEqual({})
  })
})

describe('readBackup', () => {
  it('rejects files that are not Tessera backups', async () => {
    await expect(readBackup(file('not json'))).rejects.toThrow('not JSON')
    await expect(readBackup(file({ format: 'other' }))).rejects.toBeInstanceOf(BackupError)
    await expect(
      readBackup(file(backupOf(entry({ photos: { front: 'data:text/html;base64,PGI+' } })))),
    ).rejects.toThrow('not a valid')
  })
})

describe('importBackup', () => {
  it('adds new cards with their places, favourite and photos', async () => {
    const place = { label: 'Forno', lat: 45, lon: 9 }
    const backup = backupOf(entry({ isFavorite: true, places: [place], photos: { front: PNG } }))

    const result = await importBackup(backup, [])

    expect(result).toEqual({ imported: 1, skipped: 0 })
    const [stored] = await db.cards.toArray()
    expect(stored).toMatchObject({
      storeName: 'Coop',
      isFavorite: true,
      places: [place],
      localPhotos: { front: PNG },
    })
    const ops = (await db.outbox.toArray()).map((e) => e.op.type)
    expect(ops).toEqual(['save', 'places', 'favorite', 'photo'])
  })

  it('skips cards already in the wallet and invalid numbers', async () => {
    const backup = backupOf(
      entry(),
      entry({ storeName: 'Coop copy' }),
      entry({ cardNumber: '4006381333932' }),
      entry({ barcodeFormat: 'none', cardNumber: 'X1' }),
    )

    const result = await importBackup(backup, [card({ cardNumber: '4006381333931', barcodeFormat: 'ean13' })])

    expect(result).toEqual({ imported: 1, skipped: 3 })
    expect((await db.cards.toArray()).map((c) => c.cardNumber)).toEqual(['X1'])
  })
})
