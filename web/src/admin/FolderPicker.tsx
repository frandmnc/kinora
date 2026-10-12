import { useState } from 'react'
import { api } from '../shared/api'
import { useApi } from '../shared/useApi'
import type { Folder } from '../shared/types/Folder'
import type { FolderListing } from '../shared/types/FolderListing'
import type { NewFolder } from '../shared/types/NewFolder'

/**
 * Explorador de carpetas del servidor (no de la compu desde donde se abre el Admin).
 * Se navega entrando en carpetas y se elige la actual.
 */
export function FolderPicker({
  start,
  onSelect,
  onCancel,
}: {
  start?: string | null
  onSelect: (path: string) => void
  onCancel: () => void
}) {
  const [path, setPath] = useState(start ?? '')
  const { data: listing, error, reload } = useApi<FolderListing>(
    `/api/admin/folders${path ? `?path=${encodeURIComponent(path)}` : ''}`,
  )
  const [actionError, setActionError] = useState<string>()

  async function createFolder() {
    if (!listing) return
    const name = prompt(`Nombre de la carpeta nueva dentro de ${listing.path}:`)
    if (!name) return
    try {
      const body: NewFolder = { parent: listing.path, name }
      const folder = await api<Folder>('/api/admin/folders', { method: 'POST', body })
      setPath(folder.path)
    } catch (err) {
      setActionError((err as Error).message)
    }
  }

  function go(to: string) {
    setActionError(undefined)
    setPath(to)
  }

  return (
    <div className="folder-picker">
      <div className="picker-shortcuts">
        {listing?.shortcuts.map((shortcut) => (
          <button key={shortcut.path} onClick={() => go(shortcut.path)}>
            {shortcut.name}
          </button>
        ))}
      </div>

      <div className="picker-path">
        <button disabled={!listing?.parent} onClick={() => listing?.parent && go(listing.parent)} title="Subir un nivel">
          ↑
        </button>
        <span className="mono grow">{listing?.path ?? path}</span>
        <button onClick={reload}>Actualizar</button>
      </div>

      {(error || actionError) && <p className="error">{actionError ?? error}</p>}

      <ul className="picker-folders">
        {listing?.folders.length === 0 && <li className="muted">No hay subcarpetas.</li>}
        {listing?.folders.map((folder) => (
          <li key={folder.path}>
            <button onClick={() => go(folder.path)}>📁 {folder.name}</button>
          </li>
        ))}
      </ul>

      <div className="picker-actions">
        <button onClick={createFolder} disabled={!listing}>
          Nueva carpeta…
        </button>
        <span className="grow" />
        <button onClick={onCancel}>Cancelar</button>
        <button className="primary" disabled={!listing} onClick={() => listing && onSelect(listing.path)}>
          Elegir esta carpeta
        </button>
      </div>
    </div>
  )
}
