import { useMutation, useQuery } from '@tanstack/react-query'
import { useEffect, useId, useState } from 'react'
import { linkBrand, searchBrands } from '../../api/brands'
import { Alert } from '../../components/Alert'
import { Button } from '../../components/Button'
import { describeError } from '../auth/errors'
import { INPUT_CLASS } from '../cards/Field'

/** Wait for a pause in typing before asking the server (and Wikidata). */
const SEARCH_DELAY_MS = 400
const MIN_QUERY_CHARS = 2

export interface BrandChoice {
  readonly id: string
  readonly name: string
}

interface BrandPickerProps {
  readonly value: BrandChoice | null
  readonly onChange: (brand: BrandChoice | null) => void
}

/** Links the card to a store chain, so its shops can be found nearby. */
export function BrandPicker({ value, onChange }: BrandPickerProps) {
  const [editing, setEditing] = useState(false)

  return (
    <fieldset className="flex flex-col gap-2">
      <legend className="font-medium">Store brand (optional)</legend>
      {value && !editing ? (
        <div className="flex items-center justify-between gap-2 rounded-xl bg-white px-4 py-2 ring-1 ring-brand-900/15">
          <span className="truncate">{value.name}</span>
          <span className="flex shrink-0 gap-2">
            <Button variant="secondary" onClick={() => setEditing(true)}>
              Change
            </Button>
            <Button variant="secondary" onClick={() => onChange(null)}>
              Remove
            </Button>
          </span>
        </div>
      ) : (
        <BrandSearch
          onPick={(brand) => {
            onChange(brand)
            setEditing(false)
          }}
        />
      )}
      <p className="text-sm text-brand-900/60">
        With a brand, the card shows up at the top when you are near one of its shops.
      </p>
    </fieldset>
  )
}

function BrandSearch({ onPick }: { readonly onPick: (brand: BrandChoice) => void }) {
  const inputId = useId()
  const [text, setText] = useState('')
  const [query, setQuery] = useState('')

  useEffect(() => {
    const timer = setTimeout(() => setQuery(text.trim()), SEARCH_DELAY_MS)
    return () => clearTimeout(timer)
  }, [text])

  const results = useQuery({
    queryKey: ['brand-search', query],
    queryFn: ({ signal }) => searchBrands(query, signal),
    enabled: query.length >= MIN_QUERY_CHARS,
    staleTime: 5 * 60_000,
  })
  const link = useMutation({
    mutationFn: (wikidataId: string) => linkBrand(wikidataId),
    onSuccess: (brand) => onPick({ id: brand.id, name: brand.name }),
  })

  const data = results.data
  return (
    <div className="flex flex-col gap-2">
      <label htmlFor={inputId} className="sr-only">
        Search store brand
      </label>
      <input
        id={inputId}
        type="search"
        className={INPUT_CLASS}
        placeholder="Search a chain, e.g. Esselunga"
        autoComplete="off"
        value={text}
        onChange={(event) => setText(event.target.value)}
      />
      {results.isFetching && <p className="text-sm text-brand-900/60">Searching…</p>}
      {results.isError && <Alert>{describeError(results.error)}</Alert>}
      {link.isError && <Alert>{describeError(link.error)}</Alert>}
      {data && (
        <ul
          aria-label="Brands"
          className="flex flex-col divide-y divide-brand-900/10 rounded-xl bg-white ring-1 ring-brand-900/15"
        >
          {data.linked.map((brand) => (
            <BrandOption
              key={brand.id}
              name={brand.name}
              detail={brand.locationCount > 0 ? `${brand.locationCount} shops` : 'Shops not loaded yet'}
              onSelect={() => onPick({ id: brand.id, name: brand.name })}
            />
          ))}
          {data.candidates.map((candidate) => (
            <BrandOption
              key={candidate.wikidataId}
              name={candidate.name}
              detail={candidate.description ?? candidate.wikidataId}
              busy={link.isPending && link.variables === candidate.wikidataId}
              onSelect={() => link.mutate(candidate.wikidataId)}
            />
          ))}
          {data.linked.length + data.candidates.length === 0 && (
            <li className="px-4 py-2 text-sm text-brand-900/60">No brands found.</li>
          )}
        </ul>
      )}
      {data?.remoteUnavailable && (
        <p className="text-sm text-brand-900/60">
          Wikidata is not reachable right now; showing known brands only.
        </p>
      )}
    </div>
  )
}

interface BrandOptionProps {
  readonly name: string
  readonly detail: string
  readonly busy?: boolean
  readonly onSelect: () => void
}

function BrandOption({ name, detail, busy = false, onSelect }: BrandOptionProps) {
  return (
    <li>
      <button
        type="button"
        aria-busy={busy || undefined}
        className="flex w-full min-h-11 flex-col items-start px-4 py-2 text-left hover:bg-brand-50"
        onClick={onSelect}
      >
        <span className="font-medium">{name}</span>
        <span className="text-sm text-brand-900/60">{detail}</span>
      </button>
    </li>
  )
}
