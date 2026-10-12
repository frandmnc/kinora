import { useState, type FormEvent } from 'react'
import { api } from '../../shared/api'
import { useApi } from '../../shared/useApi'
import type { Library } from '../../shared/types/Library'
import type { NewLibrary } from '../../shared/types/NewLibrary'
import type { LibraryChanges } from '../../shared/types/LibraryChanges'

export function LibrariesPage() {
  const { data: libraries, error, loading, reload } = useApi<Library[]>('/api/admin/libraries')
  const [actionError, setActionError] = useState<string>()

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
      <h1>Bibliotecas</h1>
      <p className="muted">
        Cada biblioteca es una categoría con su carpeta destino. Lo que importes se guardará ahí.
      </p>

      <CreateForm onCreate={(library) => run(() => api('/api/admin/libraries', { method: 'POST', body: library }))} />

      {actionError && <p className="error">{actionError}</p>}
      {error && <p className="error">No se pudieron cargar las bibliotecas: {error}</p>}
      {loading && !libraries && <p className="muted">Cargando…</p>}

      {libraries && libraries.length === 0 && <p className="muted">Todavía no hay bibliotecas.</p>}
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
    </>
  )
}

function CreateForm({ onCreate }: { onCreate: (library: NewLibrary) => Promise<boolean> }) {
  const [name, setName] = useState('')
  const [rootDir, setRootDir] = useState('')

  async function submit(event: FormEvent) {
    event.preventDefault()
    if (await onCreate({ name, root_dir: rootDir })) {
      setName('')
      setRootDir('')
    }
  }

  return (
    <form className="inline-form" onSubmit={submit}>
      <input placeholder="Nombre (ej. Anime)" value={name} onChange={(e) => setName(e.target.value)} required />
      <input
        className="grow mono"
        placeholder="Carpeta en el servidor (ej. ~/Movies/Kinora/Anime)"
        value={rootDir}
        onChange={(e) => setRootDir(e.target.value)}
        required
      />
      <button className="primary" type="submit">
        Crear
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
            <input value={name} onChange={(e) => setName(e.target.value)} autoFocus />
            <button type="submit">Guardar</button>
            <button type="button" onClick={() => setEditing(false)}>
              Cancelar
            </button>
          </form>
        ) : (
          library.name
        )}
      </td>
      <td className="mono">{library.root_dir}</td>
      <td className="actions">
        {!editing && (
          <>
            <button onClick={() => setEditing(true)}>Renombrar</button>
            <button
              className="danger"
              onClick={() => {
                if (confirm(`¿Eliminar la biblioteca "${library.name}"? Los archivos de la carpeta no se tocan.`)) {
                  onDelete()
                }
              }}
            >
              Eliminar
            </button>
          </>
        )}
      </td>
    </tr>
  )
}
