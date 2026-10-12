import { useState } from 'react'
import { Link } from 'react-router'
import { api } from '../../shared/api'
import { formatSize } from '../../shared/format'
import { useApi } from '../../shared/useApi'
import type { Library } from '../../shared/types/Library'
import type { MediaDirChange } from '../../shared/types/MediaDirChange'
import type { MediaDirSetting } from '../../shared/types/MediaDirSetting'
import type { MediaUsage } from '../../shared/types/MediaUsage'
import type { SetMediaDir } from '../../shared/types/SetMediaDir'
import { FolderPicker } from '../FolderPicker'

export function SettingsPage() {
  const { data: mediaDir, error, reload } = useApi<MediaDirSetting>('/api/admin/settings/media-dir')
  const { data: libraries } = useApi<Library[]>('/api/admin/libraries')
  const [picking, setPicking] = useState(false)
  // Carpeta elegida que espera la decisión de mover o no las bibliotecas.
  const [pending, setPending] = useState<string>()
  const [saving, setSaving] = useState(false)
  const [saveError, setSaveError] = useState<string>()

  async function save(path: string, change: MediaDirChange | null) {
    setSaveError(undefined)
    setSaving(true)
    try {
      const body: SetMediaDir = { path, change }
      await api('/api/admin/settings/media-dir', { method: 'PUT', body })
      setPicking(false)
      setPending(undefined)
      reload()
    } catch (err) {
      setSaveError((err as Error).message)
    } finally {
      setSaving(false)
    }
  }

  function choose(path: string) {
    setPicking(false)
    const hasLibraries = (libraries?.length ?? 0) > 0
    if (hasLibraries && mediaDir?.path && path !== mediaDir.path) setPending(path)
    else save(path, null)
  }

  return (
    <>
      <h1>Configuración</h1>

      <section className="card-section">
        <h2>Carpeta media</h2>
        <p className="muted">
          Donde Kinora guarda todo lo importado. Cada biblioteca es una subcarpeta que Kinora crea y administra.
        </p>
        {error && <p className="error">{error}</p>}
        {mediaDir && (
          <p>
            {mediaDir.path ? (
              <>
                <span className="mono">{mediaDir.path}</span>
                {!mediaDir.available && <span className="error"> (no disponible)</span>}
              </>
            ) : (
              <span className="error">Todavía no elegiste una carpeta.</span>
            )}
          </p>
        )}
        {saveError && <p className="error">{saveError}</p>}

        {pending ? (
          <MoveChoice
            from={mediaDir?.path ?? ''}
            to={pending}
            libraries={libraries ?? []}
            saving={saving}
            onChoose={(change) => save(pending, change)}
            onCancel={() => {
              setPending(undefined)
              setSaveError(undefined)
            }}
          />
        ) : picking ? (
          <FolderPicker start={mediaDir?.path} onSelect={choose} onCancel={() => setPicking(false)} />
        ) : (
          <button className="primary" onClick={() => setPicking(true)}>
            {mediaDir?.path ? 'Cambiar…' : 'Elegir carpeta…'}
          </button>
        )}

        {/* `key`: al cambiar la carpeta, el componente se vuelve a crear y mide de nuevo. */}
        {mediaDir?.available && !picking && !pending && <UsageView key={mediaDir.path} />}
      </section>
    </>
  )
}

/** Qué hacer con las carpetas de las bibliotecas al cambiar la carpeta media. */
function MoveChoice({
  from,
  to,
  libraries,
  saving,
  onChoose,
  onCancel,
}: {
  from: string
  to: string
  libraries: Library[]
  saving: boolean
  onChoose: (change: MediaDirChange) => void
  onCancel: () => void
}) {
  const folders = libraries.map((library) => library.dir).join(', ')
  return (
    <div className="move-choice">
      <p>
        Nueva carpeta: <span className="mono">{to}</span>
      </p>
      <p className="muted">
        Hay {libraries.length === 1 ? 'una biblioteca' : `${libraries.length} bibliotecas`} ({folders}). ¿Qué hacemos
        con sus carpetas?
      </p>
      <div className="choices">
        <button className="choice" disabled={saving} onClick={() => onChoose('move')}>
          <strong>Moverlas</strong>
          <span>
            Kinora mueve las carpetas desde <span className="mono">{from}</span>. Es instantáneo si la carpeta nueva
            está en el mismo disco. Entre discos distintos todavía no se puede.
          </span>
        </button>
        <button className="choice" disabled={saving} onClick={() => onChoose('already_moved')}>
          <strong>Ya las moví yo</strong>
          <span>Kinora no toca nada: solo verifica que las carpetas estén en la ubicación nueva.</span>
        </button>
      </div>
      <button onClick={onCancel} disabled={saving}>
        Cancelar
      </button>
    </div>
  )
}

/** Cuánto ocupa la carpeta media, cuánto queda en el disco y qué es cada subcarpeta. */
function UsageView() {
  const { data: usage, error, loading, reload } = useApi<MediaUsage>('/api/admin/settings/media-dir/usage')

  if (error) return <p className="error">{error}</p>
  if (!usage) return <p className="muted">{loading ? 'Midiendo…' : null}</p>

  const total = usage.disk_total ?? 0
  const free = usage.disk_available ?? 0
  const percent = (bytes: number) => (total > 0 ? `${(bytes / total) * 100}%` : '0%')

  return (
    <div className="usage">
      <div className="page-header">
        <h3>
          Ocupa {formatSize(usage.size)} <span className="muted">en {usage.files} archivos</span>
        </h3>
        <button onClick={reload}>Medir de nuevo</button>
      </div>

      {total > 0 && (
        <>
          <div className="disk-bar" title="Kinora · otros archivos del disco · libre">
            <span className="kinora" style={{ width: percent(usage.size) }} />
            <span className="other" style={{ width: percent(total - free - usage.size) }} />
          </div>
          <p className="muted small">
            <span className="dot kinora" /> Kinora {formatSize(usage.size)} · <span className="dot other" /> Otros{' '}
            {formatSize(Math.max(0, total - free - usage.size))} · Libre {formatSize(free)} de {formatSize(total)}
          </p>
        </>
      )}

      {usage.missing.length > 0 && (
        <p className="error">
          Bibliotecas sin su carpeta: {usage.missing.map((library) => library.name).join(', ')}
        </p>
      )}

      <table className="table compact">
        <thead>
          <tr>
            <th>Carpeta</th>
            <th>Es</th>
            <th>Archivos</th>
            <th>Tamaño</th>
          </tr>
        </thead>
        <tbody>
          {usage.folders.length === 0 && (
            <tr>
              <td colSpan={4} className="muted">
                La carpeta está vacía.
              </td>
            </tr>
          )}
          {usage.folders.map((folder) => (
            <tr key={folder.name}>
              <td className="mono">{folder.name}/</td>
              <td>
                {folder.role === 'library' && folder.library && (
                  <Link to="/libraries">Biblioteca «{folder.library.name}»</Link>
                )}
                {folder.role === 'inbox' && <Link to="/import">Bandeja de entrada</Link>}
                {folder.role === 'other' && <span className="muted">Sin biblioteca</span>}
              </td>
              <td>{folder.files}</td>
              <td>
                <div className="position">
                  <span>{formatSize(folder.size)}</span>
                  {usage.size > 0 && (
                    <span className="bar">
                      <span style={{ width: `${(folder.size / usage.size) * 100}%` }} />
                    </span>
                  )}
                </div>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  )
}
