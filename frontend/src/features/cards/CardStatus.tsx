import { Link } from 'react-router'
import { FullPageSpinner } from '../../components/Spinner'
import { Screen } from '../../components/Screen'

/** Loading or "not found" screen while a single card is resolved. */
export function CardStatus({ loading }: { readonly loading: boolean }) {
  if (loading) return <FullPageSpinner label="Loading card…" />
  return (
    <Screen centered>
      <p className="text-center text-lg">This card does not exist or was deleted.</p>
      <Link to="/" className="text-center font-medium text-brand-700 underline">
        Back to your cards
      </Link>
    </Screen>
  )
}
