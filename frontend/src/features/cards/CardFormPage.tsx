import { type FormEvent, useState } from 'react'
import { Link, useNavigate } from 'react-router'
import type { BarcodeFormat } from '../../api/generated/BarcodeFormat'
import type { CardInput } from '../../api/generated/CardInput'
import type { ImageKind } from '../../api/generated/ImageKind'
import { BARCODE_FORMATS } from '../../api/schemas'
import { Alert } from '../../components/Alert'
import { Button } from '../../components/Button'
import { Screen } from '../../components/Screen'
import type { LocalCard } from '../../offline/db'
import { enqueue } from '../../offline/syncEngine'
import { useMe } from '../auth/useMe'
import { type BrandChoice, BrandPicker } from '../brands/BrandPicker'
import { ScanControls } from '../scan/ScanControls'
import { CardStatus } from './CardStatus'
import { ColorPicker } from './ColorPicker'
import { Field, INPUT_CLASS } from './Field'
import { PhotoField } from './PhotoField'
import { FORMAT_INFO, checkCardNumber } from './barcodeFormats'
import { KEEP_PHOTO, type PhotoChange } from './photoChange'
import { photoSrc } from './photoSrc'
import { useCardFromRoute } from './useCardFromRoute'
import { uuidv7 } from './uuid'

type PhotoChanges = Readonly<Record<ImageKind, PhotoChange>>

export function NewCardPage() {
  const [id] = useState(uuidv7)
  return <CardForm id={id} existing={undefined} />
}

export function EditCardPage() {
  const { id, card, loading } = useCardFromRoute()
  if (!card) return <CardStatus loading={loading} />
  // Keyed by id: moving to another card must not keep this card's form state.
  return <CardForm key={id} id={id} existing={card} />
}

interface CardFormProps {
  readonly id: string
  readonly existing: LocalCard | undefined
}

function CardForm({ id, existing }: CardFormProps) {
  const navigate = useNavigate()
  const [storeName, setStoreName] = useState(existing?.storeName ?? '')
  const [format, setFormat] = useState<BarcodeFormat>(existing?.barcodeFormat ?? 'ean13')
  const [number, setNumber] = useState(existing?.cardNumber ?? '')
  const [memberName, setMemberName] = useState(existing?.memberName ?? '')
  const [color, setColor] = useState<string | null>(existing?.color ?? null)
  const [notes, setNotes] = useState(existing?.notes ?? '')
  const [expiresOn, setExpiresOn] = useState(existing?.expiresOn ?? '')
  const [brand, setBrand] = useState<BrandChoice | null>(
    existing?.brandId ? { id: existing.brandId, name: existing.brandName ?? 'Linked brand' } : null,
  )
  const [photos, setPhotos] = useState<PhotoChanges>({ front: KEEP_PHOTO, back: KEEP_PHOTO })
  const me = useMe().data
  const isOwner = !existing || existing.ownerId === me?.user.id
  const [shared, setShared] = useState(existing?.shared ?? false)
  const [submitted, setSubmitted] = useState(false)
  const [saveError, setSaveError] = useState<string | null>(null)

  const numberCheck = checkCardNumber(format, number)
  const storeNameError = storeName.trim() === '' ? 'Store name is required' : null
  const numberError = numberCheck.ok ? null : numberCheck.message

  const submit = async (event: FormEvent) => {
    event.preventDefault()
    setSubmitted(true)
    if (storeNameError || !numberCheck.ok) return
    const input: CardInput = {
      storeName: storeName.trim(),
      cardNumber: numberCheck.value,
      barcodeFormat: format,
      memberName: memberName.trim() || undefined,
      color: color ?? undefined,
      notes: notes.trim() || undefined,
      brandId: brand?.id,
      expiresOn: expiresOn || undefined,
      // Hand-marked shops are left out: they have their own op (card view).
    }
    try {
      // Saved on the device at once; the outbox sends it when there is network.
      await enqueue(id, { type: 'save', input, brandName: brand?.name ?? null })
      if (isOwner && shared !== (existing?.shared ?? false)) await enqueue(id, { type: 'share', shared })
      for (const kind of ['front', 'back'] as const) await queuePhoto(id, kind, photos[kind])
    } catch (error) {
      console.error('Saving on this device failed', error)
      setSaveError('This card could not be saved on this device. Check the free storage and try again.')
      return
    }
    await navigate(`/cards/${id}`, { replace: true })
  }

  return (
    <Screen>
      <header className="flex items-center justify-between">
        <h1 className="text-2xl font-semibold">{existing ? 'Edit card' : 'New card'}</h1>
        <Link to={existing ? `/cards/${id}` : '/'} className="min-h-11 py-2 font-medium text-brand-700">
          Cancel
        </Link>
      </header>
      <form noValidate onSubmit={(event) => void submit(event)} className="flex flex-col gap-5">
        <ScanControls
          onScanned={(scannedFormat, text) => {
            setFormat(scannedFormat)
            setNumber(text)
          }}
        />
        <Field label="Store name" error={submitted ? storeNameError : null}>
          {(props) => (
            <input
              {...props}
              className={INPUT_CLASS}
              value={storeName}
              autoComplete="off"
              maxLength={64}
              onChange={(e) => setStoreName(e.target.value)}
            />
          )}
        </Field>
        <Field label="Barcode type">
          {(props) => (
            <select
              {...props}
              className={INPUT_CLASS}
              value={format}
              onChange={(e) => setFormat(e.target.value as BarcodeFormat)}
            >
              {BARCODE_FORMATS.map((value) => (
                <option key={value} value={value}>
                  {FORMAT_INFO[value].label}
                </option>
              ))}
            </select>
          )}
        </Field>
        <Field label="Card number" error={submitted || number ? numberError : null}>
          {(props) => (
            <input
              {...props}
              className={`${INPUT_CLASS} font-mono`}
              value={number}
              inputMode={FORMAT_INFO[format].inputMode}
              autoComplete="off"
              autoCapitalize="characters"
              spellCheck={false}
              onChange={(e) => setNumber(e.target.value)}
            />
          )}
        </Field>
        <Field label="Name on the card (optional)">
          {(props) => (
            <input
              {...props}
              className={INPUT_CLASS}
              value={memberName}
              maxLength={64}
              onChange={(e) => setMemberName(e.target.value)}
            />
          )}
        </Field>
        <BrandPicker value={brand} onChange={setBrand} />
        <ColorPicker value={color} onChange={setColor} />
        <Field label="Expires on (optional)">
          {(props) => (
            <input
              {...props}
              type="date"
              className={INPUT_CLASS}
              value={expiresOn}
              onChange={(e) => setExpiresOn(e.target.value)}
            />
          )}
        </Field>
        <Field label="Notes (optional)">
          {(props) => (
            <textarea
              {...props}
              className={`${INPUT_CLASS} min-h-24`}
              value={notes}
              maxLength={1000}
              onChange={(e) => setNotes(e.target.value)}
            />
          )}
        </Field>
        {isOwner && me && me.household.members.length > 1 && (
          <label className="flex min-h-11 items-center justify-between gap-3 rounded-xl bg-white px-4 py-2 ring-1 ring-brand-900/15">
            <span>
              <span className="block font-medium">Share with {me.household.name}</span>
              <span className="block text-sm text-brand-900/60">
                Everyone in your household can use and edit it.
              </span>
            </span>
            <input
              type="checkbox"
              role="switch"
              className="size-5 accent-brand-700"
              checked={shared}
              onChange={(event) => setShared(event.target.checked)}
            />
          </label>
        )}
        <PhotoField
          label="Front"
          currentSrc={existing ? photoSrc(existing, 'front') : null}
          change={photos.front}
          onChange={(front) => setPhotos((current) => ({ ...current, front }))}
        />
        <PhotoField
          label="Back"
          currentSrc={existing ? photoSrc(existing, 'back') : null}
          change={photos.back}
          onChange={(back) => setPhotos((current) => ({ ...current, back }))}
        />
        {saveError && <Alert>{saveError}</Alert>}
        <Button type="submit">Save card</Button>
      </form>
      {existing && <DeleteCard card={existing} />}
    </Screen>
  )
}

function queuePhoto(cardId: string, kind: ImageKind, change: PhotoChange): Promise<void> {
  switch (change.type) {
    case 'keep':
      return Promise.resolve()
    case 'remove':
      return enqueue(cardId, { type: 'photo', kind, photo: null, preview: null })
    case 'replace':
      return change.photo.arrayBuffer().then((bytes) =>
        enqueue(cardId, {
          type: 'photo',
          kind,
          photo: { bytes, mime: change.photo.type },
          preview: change.preview,
        }),
      )
  }
}

function DeleteCard({ card }: { readonly card: LocalCard }) {
  const navigate = useNavigate()
  const me = useMe().data
  const [confirming, setConfirming] = useState(false)
  if (me?.user.id !== card.ownerId) return null

  const remove = async () => {
    await enqueue(card.id, { type: 'delete' })
    await navigate('/', { replace: true })
  }

  return (
    <div className="flex flex-col gap-2 border-t border-brand-900/10 pt-5">
      <Button variant="danger" onClick={() => (confirming ? void remove() : setConfirming(true))}>
        {confirming ? 'Tap again to delete for good' : 'Delete card'}
      </Button>
    </div>
  )
}
