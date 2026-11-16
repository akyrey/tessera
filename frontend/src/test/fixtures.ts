import type { CardDto } from '../api/generated/CardDto'
import type { Me } from '../api/generated/Me'
import type { PasskeyDto } from '../api/generated/PasskeyDto'

export const owner: Me = {
  user: { id: 'u-alice', displayName: 'Alice', isAdmin: true },
  household: {
    id: 'h-1',
    name: 'Home',
    role: 'owner',
    members: [
      { userId: 'u-alice', displayName: 'Alice', role: 'owner', joinedAt: 1_800_000_000 },
      { userId: 'u-bea', displayName: 'Bea', role: 'member', joinedAt: 1_800_000_100 },
    ],
  },
}

export const member: Me = {
  user: { id: 'u-bea', displayName: 'Bea', isAdmin: false },
  household: { ...owner.household, role: 'member' },
}

export const passkey = (id: string, name: string): PasskeyDto => ({
  id,
  name,
  createdAt: 1_800_000_000,
  lastUsedAt: null,
})

export const ceremony = { ceremonyId: 'c-1', options: { publicKey: { challenge: 'abc' } } }

export const card = (overrides: Partial<CardDto> = {}): CardDto => ({
  id: '0190f0c8-0000-7000-8000-000000000001',
  ownerId: 'u-alice',
  ownerName: 'Alice',
  shared: false,
  storeName: 'Esselunga',
  cardNumber: '4006381333931',
  barcodeFormat: 'ean13',
  memberName: null,
  color: null,
  notes: null,
  isFavorite: false,
  useCount: 0,
  lastUsedAt: null,
  frontImageId: null,
  backImageId: null,
  brandId: null,
  brandName: null,
  places: [],
  createdAt: 1_800_000_000,
  updatedAt: 1_800_000_000,
  ...overrides,
})
