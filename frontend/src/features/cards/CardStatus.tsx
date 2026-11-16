import { Link } from 'react-router'
import { Alert } from '../../components/Alert'
import { FullPageSpinner } from '../../components/Spinner'
import { Screen } from '../../components/Screen'
import { describeError } from '../auth/errors'
import type { useCards } from './useCards'

/** Loading, failure or "not found" screen while a single card is resolved. */
export function CardStatus({ query }: { readonly query: ReturnType<typeof useCards> }) {
  if (query.isPending) return <FullPageSpinner label="Loading card…" />
  return (
    <Screen centered>
      {query.isError ? (
        <Alert>{describeError(query.error)}</Alert>
      ) : (
        <p className="text-center text-lg">This card does not exist or was deleted.</p>
      )}
      <Link to="/" className="text-center font-medium text-brand-700 underline">
        Back to your cards
      </Link>
    </Screen>
  )
}
