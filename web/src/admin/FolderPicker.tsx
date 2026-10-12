import { useState } from 'react'
import { ArrowUp, Folder as FolderIcon, FolderPlus, HardDrive, House, RefreshCw } from 'lucide-react'
import { api } from '../shared/api'
import { useApi } from '../shared/useApi'
import type { Folder } from '../shared/types/Folder'
import type { FolderListing } from '../shared/types/FolderListing'
import type { NewFolder } from '../shared/types/NewFolder'
import { Alert } from './ui'

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
        {listing?.shortcuts.map((shortcut, i) => (
          <button key={shortcut.path} className="btn btn-sm" onClick={() => go(shortcut.path)}>
            {i === 0 ? <House /> : <HardDrive />}
            {shortcut.name}
          </button>
        ))}
      </div>

      <div className="picker-toolbar" style={{ marginTop: 10 }}>
        <button
          className="btn btn-sm btn-icon"
          disabled={!listing?.parent}
          onClick={() => listing?.parent && go(listing.parent)}
          title="Subir un nivel"
        >
          <ArrowUp />
        </button>
        <span className="mono grow truncate" title={listing?.path}>
          {listing?.path ?? path}
        </span>
        <button className="btn btn-sm btn-icon btn-ghost" onClick={reload} title="Actualizar">
          <RefreshCw />
        </button>
      </div>

      {(error || actionError) && (
        <div style={{ padding: 10 }}>
          <Alert>{actionError ?? error}</Alert>
        </div>
      )}

      <ul className="picker-folders">
        {listing?.folders.length === 0 && <li className="muted">No hay subcarpetas.</li>}
        {listing?.folders.map((folder) => (
          <li key={folder.path}>
            <button onClick={() => go(folder.path)}>
              <FolderIcon />
              <span className="truncate">{folder.name}</span>
            </button>
          </li>
        ))}
      </ul>

      <div className="picker-footer">
        <button className="btn btn-sm" onClick={createFolder} disabled={!listing}>
          <FolderPlus /> Nueva carpeta
        </button>
        <span className="grow" />
        <button className="btn btn-sm btn-ghost" onClick={onCancel}>
          Cancelar
        </button>
        <button className="btn btn-sm btn-primary" disabled={!listing} onClick={() => listing && onSelect(listing.path)}>
          Elegir esta carpeta
        </button>
      </div>
    </div>
  )
}
