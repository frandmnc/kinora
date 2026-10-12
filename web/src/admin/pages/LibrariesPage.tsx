import { useState, type FormEvent } from 'react'
import { Link } from 'react-router'
import { Check, Folder, LibraryBig, Pencil, Plus, Trash, X } from 'lucide-react'
import { api } from '../../shared/api'
import { useApi } from '../../shared/useApi'
import type { Library } from '../../shared/types/Library'
import type { NewLibrary } from '../../shared/types/NewLibrary'
import type { LibraryChanges } from '../../shared/types/LibraryChanges'
import type { MediaDirSetting } from '../../shared/types/MediaDirSetting'
import { Alert, Card, EmptyState, PageHeader } from '../ui'

export function LibrariesPage() {
  const { data: libraries, error, loading, reload } = useApi<Library[]>('/api/admin/libraries')
  const { data: mediaDir } = useApi<MediaDirSetting>('/api/admin/settings/media-dir')
  const [actionError, setActionError] = useState<string>()
  const noMediaDir = mediaDir && !mediaDir.path

  // Ejecuta una acción, muestra su error si falla y recarga la lista.
  async function run(action: () => Promise<unknown>) {
    setActionError(undefined)
    try {
      await action()
      reload()
      return true
    } catch (err) {
      setActionError((err as Error).message)
      return false
    }
  }

  return (
    <>
      <PageHeader
        title="Bibliotecas"
        description="Cada biblioteca es una categoría con su propia carpeta dentro de la carpeta media. Kinora crea la carpeta y la renombra si cambiás el nombre."
      />

      {noMediaDir && (
        <Alert tone="warning">
          Primero elegí la carpeta media en <Link to="/settings">Configuración</Link>.
        </Alert>
      )}
      {mediaDir?.path && !mediaDir.available && (
        <Alert>
          La carpeta media <span className="mono">{mediaDir.path}</span> no está disponible.
        </Alert>
      )}
      {actionError && <Alert>{actionError}</Alert>}
      {error && <Alert>No se pudieron cargar las bibliotecas: {error}</Alert>}

      <Card
        title="Tus bibliotecas"
        description={mediaDir?.path ? <span className="mono">{mediaDir.path}</span> : undefined}
        actions={
          mediaDir?.path && (
            <Link to="/settings" className="btn btn-ghost btn-sm">
              Cambiar carpeta media
            </Link>
          )
        }
        footer={
          !noMediaDir && (
            <CreateForm onCreate={(library) => run(() => api('/api/admin/libraries', { method: 'POST', body: library }))} />
          )
        }
      >
        {loading && !libraries && <p className="card-body muted">Cargando…</p>}
        {libraries?.length === 0 && (
          <EmptyState icon={LibraryBig} title="Todavía no hay bibliotecas">
            Creá una abajo, por ejemplo "Películas", "Series" o "Anime".
          </EmptyState>
        )}
        {libraries && libraries.length > 0 && (
          <table className="table">
            <thead>
              <tr>
                <th>Nombre</th>
                <th>Carpeta</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {libraries.map((library) => (
                <LibraryRow
                  key={library.id}
                  library={library}
                  onRename={(changes) =>
                    run(() => api(`/api/admin/libraries/${library.id}`, { method: 'PATCH', body: changes }))
                  }
                  onDelete={() => run(() => api(`/api/admin/libraries/${library.id}`, { method: 'DELETE' }))}
                />
              ))}
            </tbody>
          </table>
        )}
      </Card>
    </>
  )
}

function CreateForm({ onCreate }: { onCreate: (library: NewLibrary) => Promise<boolean> }) {
  const [name, setName] = useState('')

  async function submit(event: FormEvent) {
    event.preventDefault()
    if (await onCreate({ name })) setName('')
  }

  return (
    <form className="inline-form grow" onSubmit={submit}>
      <input
        className="input"
        style={{ width: 260 }}
        placeholder="Nombre de la biblioteca nueva"
        value={name}
        onChange={(e) => setName(e.target.value)}
        required
      />
      <button className="btn btn-primary" type="submit">
        <Plus /> Crear biblioteca
      </button>
    </form>
  )
}

function LibraryRow({
  library,
  onRename,
  onDelete,
}: {
  library: Library
  onRename: (changes: LibraryChanges) => Promise<boolean>
  onDelete: () => void
}) {
  const [editing, setEditing] = useState(false)
  const [name, setName] = useState(library.name)

  async function save(event: FormEvent) {
    event.preventDefault()
    if (await onRename({ name })) setEditing(false)
  }

  return (
    <tr>
      <td>
        {editing ? (
          <form className="inline-form" onSubmit={save}>
            <input className="input" value={name} onChange={(e) => setName(e.target.value)} autoFocus />
            <button className="btn btn-primary btn-sm btn-icon" type="submit" title="Guardar">
              <Check />
            </button>
            <button
              className="btn btn-sm btn-icon"
              type="button"
              title="Cancelar"
              onClick={() => {
                setName(library.name)
                setEditing(false)
              }}
            >
              <X />
            </button>
          </form>
        ) : (
          <strong>{library.name}</strong>
        )}
      </td>
      <td>
        <span className="row secondary">
          <Folder size={15} />
          <span className="mono">{library.dir}/</span>
        </span>
      </td>
      <td className="actions-cell">
        {!editing && (
          <>
            <button className="btn btn-ghost btn-sm" onClick={() => setEditing(true)}>
              <Pencil /> Renombrar
            </button>
            <button
              className="btn btn-ghost btn-sm btn-danger"
              onClick={() => {
                if (confirm(`¿Eliminar la biblioteca "${library.name}"? La carpeta y sus archivos quedan en el disco.`)) {
                  onDelete()
                }
              }}
            >
              <Trash /> Eliminar
            </button>
          </>
        )}
      </td>
    </tr>
  )
}
