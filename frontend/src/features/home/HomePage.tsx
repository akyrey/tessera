import { Link } from 'react-router'
import { Screen } from '../../components/Screen'
import { useMe } from '../auth/useMe'

export function HomePage() {
  const me = useMe().data

  return (
    <Screen>
      <header className="flex items-center justify-between">
        <div>
          <p className="text-sm text-brand-900/60">{me?.household.name}</p>
          <h1 className="text-2xl font-semibold">Hi, {me?.user.displayName}</h1>
        </div>
        <Link
          to="/settings"
          className="rounded-xl px-3 py-2 font-medium text-brand-700 ring-1 ring-brand-900/15 hover:bg-white"
        >
          Settings
        </Link>
      </header>
      <section className="flex flex-1 flex-col items-center justify-center gap-2 rounded-2xl border-2 border-dashed border-brand-900/15 p-8 text-center">
        <h2 className="text-lg font-semibold">No cards yet</h2>
        <p className="text-brand-900/60">Adding loyalty cards arrives in the next milestone.</p>
      </section>
    </Screen>
  )
}
