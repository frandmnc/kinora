import { useState } from 'react'
import { Folder, Trash2 } from 'lucide-react'
import { api } from '../../shared/api'
import { useApi } from '../../shared/useApi'
import { formatSize } from '../../shared/format'
import type { TrashBatch } from '../../shared/types/TrashBatch'
import type { TrashLibrary } from '../../shared/types/TrashLibrary'
import type { TrashView } from '../../shared/types/TrashView'
import { Alert, Card, ConfirmDialog, EmptyState, PageHeader } from '../ui'

/** Lo que pide confirmación: vaciar todo, o una tanda de una biblioteca. */
type Pending = { kind: 'all' } | { kind: 'batch'; library: TrashLibrary; batch: TrashBatch }

/**
 * La papelera: lo que Kinora sacó de las bibliotecas (contenido eliminado, versiones quitadas,
 * archivos reemplazados) sigue en el disco hasta que se borra acá.
 */
export function TrashPage() {
  const { data: trash, error, loading, reload } = useApi<TrashView>('/api/admin/trash')
  const [pending, setPending] = useState<Pending>()
  const [busy, setBusy] = useState(false)
  const [actionError, setActionError] = useState<string>()
  const libraries = trash?.libraries.filter((l) => l.batches.length > 0) ?? []

  async function confirm() {
    if (!pending) return
    setBusy(true)
    setActionError(undefined)
    try {
      const path =
        pending.kind === 'all'
          ? '/api/admin/trash'
          : `/api/admin/trash/${pending.library.library_id}/${encodeURIComponent(pending.batch.name)}`
      await api(path, { method: 'DELETE' })
      setPending(undefined)
      reload()
    } catch (err) {
      setActionError((err as Error).message)
    } finally {
      setBusy(false)
    }
  }

  const files = (items: { files: number }[]) => items.reduce((sum, item) => sum + item.files, 0)
  const allFiles = libraries.flatMap((l) => l.batches.flatMap((b) => b.items))

  return (
    <>
      <PageHeader
        title="Papelera"
        description="Lo que Kinora saca de las bibliotecas (contenido eliminado, versiones quitadas, archivos reemplazados) no se borra enseguida: sigue ocupando lugar en el disco hasta que lo borrás acá."
        actions={
          trash &&
          trash.size > 0 && (
            <button className="btn btn-danger-solid" onClick={() => setPending({ kind: 'all' })}>
              <Trash2 /> Vaciar papelera ({formatSize(trash.size)})
            </button>
          )
        }
      />
      {error && <Alert>No se pudo leer la papelera: {error}</Alert>}
      {loading && !trash && <p className="muted">Cargando…</p>}

      {trash && libraries.length === 0 && (
        <div className="card">
          <EmptyState icon={Trash2} title="La papelera está vacía">
            Cuando elimines contenido o reemplaces archivos, lo que salga de las bibliotecas aparece acá.
          </EmptyState>
        </div>
      )}

      {libraries.map((library) => (
        <Card key={library.library_id} title={library.name} description={formatSize(library.size)}>
          {library.batches.map((batch) => (
            <div key={batch.name} className="trash-batch">
              <div className="trash-batch-head">
                <strong>{batch.deleted_at ? when(batch.deleted_at) : batch.name}</strong>
                <span className="small muted">
                  {plural(files(batch.items), 'archivo', 'archivos')} · {formatSize(batch.size)}
                </span>
                <span className="grow" />
                <button
                  className="btn btn-ghost btn-sm btn-icon btn-danger"
                  title="Borrar para siempre"
                  aria-label={`Borrar para siempre lo del ${batch.name}`}
                  onClick={() => setPending({ kind: 'batch', library, batch })}
                >
                  <Trash2 />
                </button>
              </div>
              {batch.items.map((item) => (
                <div key={item.name} className="trash-item small">
                  <Folder className="icon" />
                  <span className="mono grow truncate">{item.name}</span>
                  <span className="muted">
                    {plural(item.files, 'archivo', 'archivos')} · {formatSize(item.size)}
                  </span>
                </div>
              ))}
            </div>
          ))}
        </Card>
      ))}

      {pending && (
        <ConfirmDialog
          title={pending.kind === 'all' ? '¿Vaciar la papelera?' : '¿Borrar para siempre?'}
          confirmLabel={pending.kind === 'all' ? 'Vaciar' : 'Borrar'}
          danger
          busy={busy}
          onConfirm={confirm}
          onCancel={() => {
            setPending(undefined)
            setActionError(undefined)
          }}
        >
          {pending.kind === 'all' ? (
            <p>
              Se borran del disco <strong>{plural(files(allFiles), 'archivo', 'archivos')}</strong> (
              {formatSize(trash?.size ?? 0)}) de {plural(libraries.length, 'biblioteca', 'bibliotecas')}.
            </p>
          ) : (
            <p>
              Se borra del disco lo que salió de <strong>{pending.library.name}</strong>{' '}
              {pending.batch.deleted_at ? when(pending.batch.deleted_at).toLowerCase() : pending.batch.name}:{' '}
              {pending.batch.items.map((item) => item.name).join(', ')} (
              {plural(files(pending.batch.items), 'archivo', 'archivos')}, {formatSize(pending.batch.size)}).
            </p>
          )}
          <p className="muted">No se puede deshacer.</p>
          {actionError && <Alert>{actionError}</Alert>}
        </ConfirmDialog>
      )}
    </>
  )
}

function plural(count: number, one: string, many: string) {
  return `${count} ${count === 1 ? one : many}`
}

/** "Hoy, 10:49", "Ayer, 22:15" o "Lunes 12 de octubre, 09:00". */
function when(seconds: number) {
  const date = new Date(seconds * 1000)
  const time = date.toLocaleTimeString('es-AR', { hour: '2-digit', minute: '2-digit', hour12: false })
  const midnight = (d: Date) => new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime()
  const days = Math.round((midnight(new Date()) - midnight(date)) / 86_400_000)
  if (days === 0) return `Hoy, ${time}`
  if (days === 1) return `Ayer, ${time}`
  const day = date.toLocaleDateString('es-AR', { weekday: 'long', day: 'numeric', month: 'long' })
  return `${day.charAt(0).toUpperCase()}${day.slice(1)}, ${time}`
}
