import { useEffect, useState } from 'react'
import { Link } from 'react-router'
import { FolderCheck, FolderInput, FolderOpen, HardDrive, LoaderCircle, RefreshCw } from 'lucide-react'
import { api } from '../../shared/api'
import { formatSize } from '../../shared/format'
import { useApi } from '../../shared/useApi'
import type { Library } from '../../shared/types/Library'
import type { MediaDirChange } from '../../shared/types/MediaDirChange'
import type { MediaDirSetting } from '../../shared/types/MediaDirSetting'
import type { MediaUsage } from '../../shared/types/MediaUsage'
import type { SetMediaDir } from '../../shared/types/SetMediaDir'
import type { TransferStatus } from '../../shared/types/TransferStatus'
import { FolderPicker } from '../FolderPicker'
import { AccountCard } from '../AccountCard'
import { UpdatesCard } from '../UpdatesCard'
import { TmdbCard } from '../TmdbCard'
import { OpenSubtitlesCard } from '../OpenSubtitlesCard'
import { Alert, Badge, Card, Meter, PageHeader } from '../ui'

export function SettingsPage() {
  const { data: mediaDir, error, reload } = useApi<MediaDirSetting>('/api/admin/settings/media-dir')
  const { data: libraries } = useApi<Library[]>('/api/admin/libraries')
  const [picking, setPicking] = useState(false)
  // Carpeta elegida que espera la decisión de mover o no las bibliotecas.
  const [pending, setPending] = useState<string>()
  const [saving, setSaving] = useState(false)
  const [saveError, setSaveError] = useState<string>()
  // Mostrar el avance de una copia a otro disco (la que se acaba de empezar o una que ya estaba).
  const [watchMove, setWatchMove] = useState(false)

  async function save(path: string, change: MediaDirChange | null) {
    setSaveError(undefined)
    setSaving(true)
    try {
      const body: SetMediaDir = { path, change }
      const result = await api<MediaDirSetting>('/api/admin/settings/media-dir', { method: 'PUT', body })
      setPicking(false)
      setPending(undefined)
      if (result.moving) setWatchMove(true)
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
      <PageHeader title="Configuración" />

      <Card
        title="Carpeta media"
        description="donde Kinora guarda todo lo importado; cada biblioteca es una subcarpeta"
        actions={
          !picking &&
          !pending &&
          !mediaDir?.moving && (
            <button className="btn btn-primary" onClick={() => setPicking(true)}>
              <FolderOpen /> {mediaDir?.path ? 'Cambiar' : 'Elegir carpeta'}
            </button>
          )
        }
      >
        <div className="card-body" style={{ display: 'flex', flexDirection: 'column', gap: 14 }}>
          {error && <Alert>{error}</Alert>}
          {mediaDir &&
            (mediaDir.path ? (
              <div className="row">
                <span className="path-chip">
                  <HardDrive />
                  {mediaDir.path}
                </span>
                {mediaDir.available ? (
                  <Badge tone="good">Disponible</Badge>
                ) : (
                  <Badge tone="critical">No disponible</Badge>
                )}
              </div>
            ) : (
              <Alert tone="warning">Todavía no elegiste una carpeta. Sin ella no se pueden crear bibliotecas.</Alert>
            ))}
          {saveError && <Alert>{saveError}</Alert>}

          {(watchMove || mediaDir?.moving) && (
            <TransferView onFinished={reload} onClose={() => setWatchMove(false)} />
          )}
          {pending && (
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
          )}
          {picking && <FolderPicker start={mediaDir?.path} onSelect={choose} onCancel={() => setPicking(false)} />}
        </div>
      </Card>

      {/* `key`: al cambiar la carpeta, el componente se vuelve a crear y mide de nuevo. */}
      {mediaDir?.available && <UsageView key={mediaDir.path} />}

      <TmdbCard />

      <OpenSubtitlesCard />

      <AccountCard />

      <UpdatesCard />
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
    <>
      <Alert tone="info">
        Nueva carpeta: <span className="mono">{to}</span>. Hay{' '}
        {libraries.length === 1 ? 'una biblioteca' : `${libraries.length} bibliotecas`} ({folders}). ¿Qué hacemos con
        sus carpetas?
      </Alert>
      <div className="choices">
        <button className="choice" disabled={saving} onClick={() => onChoose('move')}>
          <strong>
            <FolderInput /> Moverlas
          </strong>
          <span>
            Kinora mueve las carpetas desde <span className="mono">{from}</span>. En el mismo disco es instantáneo. En
            otro disco las copia (puede tardar) y recién al terminar cambia de carpeta; mientras tanto se puede seguir
            mirando.
          </span>
        </button>
        <button className="choice" disabled={saving} onClick={() => onChoose('already_moved')}>
          <strong>
            <FolderCheck /> Ya las moví yo
          </strong>
          <span>Kinora no toca nada: solo verifica que las carpetas estén en la ubicación nueva.</span>
        </button>
      </div>
      <div>
        <button className="btn btn-ghost" onClick={onCancel} disabled={saving}>
          Cancelar
        </button>
      </div>
    </>
  )
}

/** El avance de la copia de las bibliotecas a otro disco. Se actualiza cada segundo mientras dura. */
/** `onFinished` tiene que ser estable (como el `reload` de `useApi`): se llama al terminar. */
function TransferView({ onFinished, onClose }: { onFinished: () => void; onClose: () => void }) {
  const { data: transfer, reload } = useApi<TransferStatus>('/api/admin/settings/media-dir/move')
  const [error, setError] = useState<string>()
  const busy = transfer?.state === 'copying' || transfer?.state === 'finishing'

  useEffect(() => {
    if (!busy) return
    const timer = setInterval(reload, 1000)
    return () => clearInterval(timer)
  }, [busy, reload])

  // Al terminar (bien, mal o cancelada) se vuelve a pedir la carpeta media: puede haber cambiado.
  const finished = transfer && !busy
  useEffect(() => {
    if (finished) onFinished()
  }, [finished, onFinished])

  if (!transfer || transfer.state === 'idle') return null
  const share = transfer.total_bytes > 0 ? transfer.copied_bytes / transfer.total_bytes : 0
  const elapsed = transfer.started_at ? Date.now() / 1000 - transfer.started_at : 0
  const remaining = share > 0.02 ? (elapsed / share) * (1 - share) : null

  async function cancel() {
    if (!confirm('¿Cancelar la copia? Se borra lo copiado y todo queda como estaba.')) return
    try {
      await api('/api/admin/settings/media-dir/move/cancel', { method: 'POST' })
      reload()
    } catch (err) {
      setError((err as Error).message)
    }
  }

  if (transfer.state === 'done') {
    return (
      <Alert tone="info">
        Listo: las bibliotecas se copiaron a <span className="mono">{transfer.to}</span> y se borraron del disco anterior.{' '}
        <button className="btn btn-sm" onClick={onClose}>
          Cerrar
        </button>
      </Alert>
    )
  }
  if (transfer.state === 'failed' || transfer.state === 'cancelled') {
    return (
      <Alert tone={transfer.state === 'failed' ? 'critical' : 'warning'}>
        {transfer.state === 'failed' ? `No se pudo copiar: ${transfer.error}.` : 'Copia cancelada.'} Se borró lo
        copiado; las bibliotecas siguen en <span className="mono">{transfer.from}</span>.{' '}
        <button className="btn btn-sm" onClick={onClose}>
          Cerrar
        </button>
      </Alert>
    )
  }
  return (
    <div className="transfer">
      <div className="row">
        <LoaderCircle className="spin" size={16} />
        <strong className="grow">
          {transfer.state === 'finishing' ? 'Terminando…' : `Copiando a ${transfer.to}`}
        </strong>
        <button className="btn btn-ghost btn-sm btn-danger" onClick={cancel} disabled={transfer.state !== 'copying'}>
          Cancelar
        </button>
      </div>
      <Meter value={share} label="Avance de la copia" />
      <div className="small muted row">
        <span className="grow mono truncate">{transfer.current}</span>
        <span>
          {formatSize(transfer.copied_bytes)} de {formatSize(transfer.total_bytes)}
          {remaining !== null && ` · faltan ${formatRemaining(remaining)}`}
        </span>
      </div>
      <div className="small muted">
        Se puede seguir mirando. Importar, la cola y los cambios del Admin esperan a que termine.
      </div>
      {error && <Alert>{error}</Alert>}
    </div>
  )
}

function formatRemaining(seconds: number) {
  if (seconds < 60) return `${Math.max(1, Math.round(seconds))} s`
  const minutes = Math.round(seconds / 60)
  return minutes < 60 ? `${minutes} min` : `${Math.floor(minutes / 60)} h ${minutes % 60} min`
}

/** Cuánto ocupa la carpeta media, cuánto queda en el disco y qué es cada subcarpeta. */
function UsageView() {
  const { data: usage, error, loading, reload } = useApi<MediaUsage>('/api/admin/settings/media-dir/usage')

  const total = usage?.disk_total ?? 0
  const free = usage?.disk_available ?? 0
  const other = usage ? Math.max(0, total - free - usage.size) : 0
  const share = (bytes: number) => (total > 0 ? `${(bytes / total) * 100}%` : '0%')

  return (
    <Card
      title="Espacio"
      description={usage ? `${formatSize(usage.size)} en ${usage.files} archivos` : loading ? 'midiendo…' : undefined}
      actions={
        <button className="btn btn-sm" onClick={reload} disabled={loading}>
          <RefreshCw /> Medir de nuevo
        </button>
      }
    >
      {error && (
        <div className="card-body">
          <Alert>{error}</Alert>
        </div>
      )}
      {usage && total > 0 && (
        <div className="card-body">
          {/* Barra apilada del disco: Kinora, otros archivos y libre, con leyenda. */}
          <div className="stack-bar" role="img" aria-label="Uso del disco">
            <span className="seg-kinora" style={{ width: share(usage.size) }} />
            <span className="seg-other" style={{ width: share(other) }} />
            <span className="seg-free" />
          </div>
          <div className="legend">
            <span>
              <i className="swatch kinora" /> Kinora <b>{formatSize(usage.size)}</b>
            </span>
            <span>
              <i className="swatch other" /> Otros archivos <b>{formatSize(other)}</b>
            </span>
            <span>
              <i className="swatch free" /> Libre <b>{formatSize(free)}</b>
            </span>
            <span className="muted">Disco de {formatSize(total)}</span>
          </div>
        </div>
      )}

      {usage && usage.missing.length > 0 && (
        <div className="card-body" style={{ paddingTop: 0 }}>
          <Alert>Bibliotecas sin su carpeta: {usage.missing.map((library) => library.name).join(', ')}</Alert>
        </div>
      )}

      {usage && (
        <table className="table">
          <thead>
            <tr>
              <th>Carpeta</th>
              <th>Es</th>
              <th className="num">Archivos</th>
              <th className="num">Tamaño</th>
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
                    <Link to="/libraries">
                      <Badge tone="accent">Biblioteca “{folder.library.name}”</Badge>
                    </Link>
                  )}
                  {folder.role === 'inbox' && (
                    <Link to="/import">
                      <Badge>Bandeja de entrada</Badge>
                    </Link>
                  )}
                  {folder.role === 'other' && <span className="muted">Sin biblioteca</span>}
                </td>
                <td className="num">{folder.files}</td>
                <td>
                  <div className="size-cell">
                    <span>{formatSize(folder.size)}</span>
                    <div className="meter meter-sm" aria-hidden="true">
                      <span style={{ width: usage.size > 0 ? `${(folder.size / usage.size) * 100}%` : 0 }} />
                    </div>
                  </div>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </Card>
  )
}
