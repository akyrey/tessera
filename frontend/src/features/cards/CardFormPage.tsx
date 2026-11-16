import { useMutation, useQueryClient } from '@tanstack/react-query'
import { type FormEvent, type ReactNode, useState } from 'react'
import { Link, useNavigate, useParams } from 'react-router'
import { deleteCard } from '../../api/cards'
import type { BarcodeFormat } from '../../api/generated/BarcodeFormat'
import type { CardDto } from '../../api/generated/CardDto'
import { BARCODE_FORMATS } from '../../api/schemas'
import { Alert } from '../../components/Alert'
import { Button } from '../../components/Button'
import { Screen } from '../../components/Screen'
import { describeError } from '../auth/errors'
import { useMe } from '../auth/useMe'
import { CardStatus } from './CardStatus'
import { ColorPicker } from './ColorPicker'
import { PhotoField } from './PhotoField'
import { KEEP_PHOTO } from './photoChange'
import { FORMAT_INFO, checkCardNumber } from './barcodeFormats'
import { type PhotoChanges, saveCardWithPhotos } from './saveCardWithPhotos'
import { CARDS_QUERY_KEY, forgetCard, storeCard } from './useCards'
import { useCardFromRoute } from './useCardFromRoute'
import { uuidv7 } from './uuid'

const INPUT =
  'min-h-11 rounded-xl bg-white px-4 py-2 ring-1 ring-brand-900/15 focus:ring-2 focus:ring-brand-700 focus:outline-none'

export function NewCardPage() {
  const [id] = useState(uuidv7)
  return <CardForm id={id} existing={undefined} />
}

export function EditCardPage() {
  const { card, query } = useCardFromRoute()
  const { id } = useParams()
  if (!card || !id) return <CardStatus query={query} />
  // Keyed by id: moving to another card must not keep this card's form state.
  return <CardForm key={id} id={id} existing={card} />
}

interface CardFormProps {
  readonly id: string
  readonly existing: CardDto | undefined
}

function CardForm({ id, existing }: CardFormProps) {
  const navigate = useNavigate()
  const queryClient = useQueryClient()
  const [storeName, setStoreName] = useState(existing?.storeName ?? '')
  const [format, setFormat] = useState<BarcodeFormat>(existing?.barcodeFormat ?? 'ean13')
  const [number, setNumber] = useState(existing?.cardNumber ?? '')
  const [memberName, setMemberName] = useState(existing?.memberName ?? '')
  const [color, setColor] = useState<string | null>(existing?.color ?? null)
  const [notes, setNotes] = useState(existing?.notes ?? '')
  const [photos, setPhotos] = useState<PhotoChanges>({ front: KEEP_PHOTO, back: KEEP_PHOTO })
  const [submitted, setSubmitted] = useState(false)

  const numberCheck = checkCardNumber(format, number)
  const storeNameError = storeName.trim() === '' ? 'Store name is required' : null
  const numberError = numberCheck.ok ? null : numberCheck.message

  const save = useMutation({
    mutationFn: () =>
      saveCardWithPhotos(
        id,
        {
          storeName: storeName.trim(),
          cardNumber: numberCheck.ok ? numberCheck.value : number,
          barcodeFormat: format,
          memberName: memberName.trim() || undefined,
          color: color ?? undefined,
          notes: notes.trim() || undefined,
        },
        photos,
      ),
    onSuccess: async (card) => {
      storeCard(queryClient, card)
      await navigate(`/cards/${card.id}`, { replace: true })
    },
    // The card may be saved even if a photo upload then failed.
    onError: () => queryClient.invalidateQueries({ queryKey: CARDS_QUERY_KEY }),
  })

  const submit = (event: FormEvent) => {
    event.preventDefault()
    setSubmitted(true)
    if (storeNameError || numberError) return
    save.mutate()
  }

  return (
    <Screen>
      <header className="flex items-center justify-between">
        <h1 className="text-2xl font-semibold">{existing ? 'Edit card' : 'New card'}</h1>
        <Link to={existing ? `/cards/${id}` : '/'} className="min-h-11 py-2 font-medium text-brand-700">
          Cancel
        </Link>
      </header>
      <form noValidate onSubmit={submit} className="flex flex-col gap-5">
        <Field label="Store name" error={submitted ? storeNameError : null}>
          {(props) => (
            <input
              {...props}
              className={INPUT}
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
              className={INPUT}
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
              className={`${INPUT} font-mono`}
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
              className={INPUT}
              value={memberName}
              maxLength={64}
              onChange={(e) => setMemberName(e.target.value)}
            />
          )}
        </Field>
        <ColorPicker value={color} onChange={setColor} />
        <Field label="Notes (optional)">
          {(props) => (
            <textarea
              {...props}
              className={`${INPUT} min-h-24`}
              value={notes}
              maxLength={1000}
              onChange={(e) => setNotes(e.target.value)}
            />
          )}
        </Field>
        <PhotoField
          label="Front"
          currentImageId={existing?.frontImageId ?? null}
          change={photos.front}
          onChange={(front) => setPhotos((current) => ({ ...current, front }))}
        />
        <PhotoField
          label="Back"
          currentImageId={existing?.backImageId ?? null}
          change={photos.back}
          onChange={(back) => setPhotos((current) => ({ ...current, back }))}
        />
        {save.isError && <Alert>{describeError(save.error)}</Alert>}
        <Button type="submit" busy={save.isPending}>
          Save card
        </Button>
      </form>
      {existing && <DeleteCard card={existing} onDeleted={() => forgetCard(queryClient, existing.id)} />}
    </Screen>
  )
}

interface FieldProps {
  readonly label: string
  readonly error?: string | null
  readonly children: (props: {
    id: string
    'aria-invalid'?: boolean
    'aria-describedby'?: string
  }) => ReactNode
}

function Field({ label, error, children }: FieldProps) {
  const id = `field-${label.toLowerCase().replace(/\W+/g, '-')}`
  const errorId = `${id}-error`
  return (
    <div className="flex flex-col gap-1">
      <label htmlFor={id} className="font-medium">
        {label}
      </label>
      {children({ id, ...(error ? { 'aria-invalid': true, 'aria-describedby': errorId } : {}) })}
      {error && (
        <p id={errorId} className="text-sm text-red-700">
          {error}
        </p>
      )}
    </div>
  )
}

function DeleteCard({ card, onDeleted }: { readonly card: CardDto; readonly onDeleted: () => void }) {
  const navigate = useNavigate()
  const me = useMe().data
  const [confirming, setConfirming] = useState(false)
  const remove = useMutation({
    mutationFn: () => deleteCard(card.id),
    onSuccess: async () => {
      onDeleted()
      await navigate('/', { replace: true })
    },
  })
  if (me?.user.id !== card.ownerId) return null

  return (
    <div className="flex flex-col gap-2 border-t border-brand-900/10 pt-5">
      {remove.isError && <Alert>{describeError(remove.error)}</Alert>}
      <Button
        variant="danger"
        busy={remove.isPending}
        onClick={() => (confirming ? remove.mutate() : setConfirming(true))}
      >
        {confirming ? 'Tap again to delete for good' : 'Delete card'}
      </Button>
    </div>
  )
}
