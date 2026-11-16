import { useMutation } from '@tanstack/react-query'
import { useId } from 'react'
import { Alert } from '../../components/Alert'
import { Button } from '../../components/Button'
import { Section } from '../../components/Section'
import { db, readMeta } from '../../offline/db'
import { buildBackup, downloadBackup, ownCards } from './exportCards'
import { BackupError, importBackup, readBackup } from './importCards'

export function BackupSection() {
  const inputId = useId()
  const exportCards = useMutation({
    mutationFn: async () => {
      const me = await readMeta('me')
      if (!me) throw new Error('No signed-in user to export cards for')
      downloadBackup(await buildBackup(ownCards(await db.cards.toArray(), me.user.id)))
    },
  })
  const importCards = useMutation({
    mutationFn: async (file: File) => importBackup(await readBackup(file), await db.cards.toArray()),
  })

  const importError = importCards.error
  return (
    <Section title="Backup">
      <p className="text-sm text-brand-900/60">
        Export your cards (with photos) to a file, or add cards from such a file. The file contains your card
        numbers: keep it somewhere private.
      </p>
      <Button variant="secondary" busy={exportCards.isPending} onClick={() => exportCards.mutate()}>
        Export cards
      </Button>
      <label
        htmlFor={inputId}
        aria-busy={importCards.isPending || undefined}
        className="inline-flex min-h-11 cursor-pointer items-center justify-center rounded-xl bg-white px-4 font-medium ring-1 ring-brand-900/15 hover:bg-brand-50"
      >
        {importCards.isPending ? 'Importing…' : 'Import cards'}
      </label>
      <input
        id={inputId}
        type="file"
        accept="application/json,.json"
        aria-label="Backup file"
        className="sr-only"
        onChange={(event) => {
          const file = event.target.files?.[0]
          if (file) importCards.mutate(file)
          event.target.value = ''
        }}
      />
      {importCards.data && (
        <p role="status" className="text-sm">
          Imported {importCards.data.imported} {importCards.data.imported === 1 ? 'card' : 'cards'}
          {importCards.data.skipped > 0 &&
            `, skipped ${importCards.data.skipped} already in your wallet or invalid`}
          .
        </p>
      )}
      {exportCards.isError && <Alert>The export failed. Please try again.</Alert>}
      {importError && (
        <Alert>
          {importError instanceof BackupError ? importError.message : 'The import failed. Please try again.'}
        </Alert>
      )}
    </Section>
  )
}
