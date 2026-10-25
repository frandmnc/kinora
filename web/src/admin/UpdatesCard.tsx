import { useEffect, useState, type ReactNode } from 'react'
import { Download, ExternalLink, RefreshCw } from 'lucide-react'
import { api } from '../shared/api'
import { useApi } from '../shared/useApi'
import type { Health } from '../shared/types/Health'
import type { InstallUpdate } from '../shared/types/InstallUpdate'
import type { UpdateStatus } from '../shared/types/UpdateStatus'
import { Alert, Badge, Card } from './ui'

/** Dónde se recuerda si se cuentan las versiones de prueba. */
const PRERELEASE_PREFERENCE = 'kinora.updates.prerelease'
/** Cuánto esperar a que Kinora vuelva con la versión nueva antes de avisar que tarda. */
const RESTART_TIMEOUT_MS = 3 * 60_000

function readPrerelease() {
  try {
    return localStorage.getItem(PRERELEASE_PREFERENCE) !== 'no'
  } catch {
    return true
  }
}

/**
 * Actualizaciones: la versión instalada y la última publicada en GitHub. Si hay una más nueva,
 * sus notas y el botón para instalarla: el servidor la baja, la verifica y se reinicia con ella
 * (lo hace la app que lo supervisa, en la barra de menú de la Mac o la bandeja de Windows).
 */
export function UpdatesCard() {
  const [prerelease, setPrerelease] = useState(readPrerelease)
  const { data: status, error, loading, reload } = useApi<UpdateStatus>(
    `/api/admin/updates?prerelease=${prerelease}`,
  )
  const [installing, setInstalling] = useState<string>()
  const [installError, setInstallError] = useState<string>()
  const [slow, setSlow] = useState(false)

  function choosePrerelease(value: boolean) {
    setPrerelease(value)
    try {
      localStorage.setItem(PRERELEASE_PREFERENCE, value ? 'yes' : 'no')
    } catch {
      // Sin almacenamiento: vale para esta visita.
    }
  }

  async function install(version: string) {
    setInstallError(undefined)
    setInstalling(version)
    try {
      const body: InstallUpdate = { version }
      await api('/api/admin/updates/install', { method: 'POST', body })
    } catch (err) {
      setInstalling(undefined)
      setInstallError((err as Error).message)
    }
  }

  // Mientras se instala, el servidor se apaga y vuelve: cuando responde con la versión nueva,
  // la página se recarga (con el Admin nuevo).
  useEffect(() => {
    if (!installing) return
    const started = Date.now()
    const timer = setInterval(async () => {
      if (Date.now() - started > RESTART_TIMEOUT_MS) setSlow(true)
      try {
        const health = await api<Health>('/api/health')
        if (health.version === installing) window.location.reload()
      } catch {
        // Todavía apagado: se sigue esperando.
      }
    }, 2000)
    return () => clearInterval(timer)
  }, [installing])

  const latest = status?.latest

  return (
    <Card
      title="Actualizaciones"
      description={status ? `versión instalada: ${status.current}` : undefined}
      actions={
        !installing && (
          <button className="btn btn-sm btn-ghost" onClick={reload} disabled={loading}>
            <RefreshCw /> Buscar
          </button>
        )
      }
    >
      <div className="card-body updates">
        {installing ? (
          <Alert tone="info">
            Bajando e instalando Kinora {installing}. Kinora se reinicia, y esta página se recarga sola cuando vuelve.
            {slow && ' Está tardando más de lo normal: mirá el ícono de Kinora en la compu donde corre.'}
          </Alert>
        ) : error ? (
          <Alert>No se pudo consultar GitHub: {error}</Alert>
        ) : !status ? (
          <p className="muted">Buscando…</p>
        ) : status.update_available && latest ? (
          <>
            <p className="updates-headline">
              Hay una versión nueva: <strong>Kinora {latest.version}</strong>
              {latest.prerelease && <Badge tone="warning">de prueba</Badge>}
              <span className="muted"> del {formatDate(latest.published)}</span>
            </p>
            <ReleaseNotes text={latest.notes} />
            {status.cannot_install ? (
              <Alert tone="warning">
                {status.cannot_install}.{' '}
                <a href={latest.url} target="_blank" rel="noreferrer">
                  Bajarla de GitHub <ExternalLink />
                </a>
              </Alert>
            ) : (
              <div className="row">
                <button className="btn btn-primary" onClick={() => install(latest.version)}>
                  <Download /> Actualizar a {latest.version}
                </button>
                <span className="small muted">
                  Kinora se reinicia: quien esté mirando algo se corta un momento, y una conversión en curso sigue
                  después.
                </span>
              </div>
            )}
          </>
        ) : (
          <p>
            Estás en la última versión
            {latest && latest.version === status.current && (
              <>
                {' '}
                (
                <a href={latest.url} target="_blank" rel="noreferrer">
                  qué trae <ExternalLink />
                </a>
                )
              </>
            )}
            .
          </p>
        )}
        {installError && <Alert>{installError}</Alert>}
        {!installing && (
          <label className="check-row small">
            <input type="checkbox" checked={prerelease} onChange={(e) => choosePrerelease(e.target.checked)} />
            Incluir las versiones de prueba
          </label>
        )}
      </div>
    </Card>
  )
}

/** "2026-10-24" → "24 de octubre de 2026". */
function formatDate(day: string) {
  const [year, month, date] = day.split('-').map(Number)
  return new Date(year, month - 1, date).toLocaleDateString('es-AR', { day: 'numeric', month: 'long', year: 'numeric' })
}

/**
 * Las notas de la versión (el CHANGELOG, en Markdown): párrafos, listas con "-" y negritas con
 * "**". Nada más hace falta.
 */
function ReleaseNotes({ text }: { text: string }) {
  const blocks: ReactNode[] = []
  let items: string[] = []
  const flush = () => {
    if (items.length > 0) {
      blocks.push(
        <ul key={blocks.length}>
          {items.map((item, i) => (
            <li key={i}>{inline(item)}</li>
          ))}
        </ul>,
      )
      items = []
    }
  }
  for (const line of text.split('\n')) {
    const trimmed = line.trim()
    if (trimmed.startsWith('- ')) {
      items.push(trimmed.slice(2))
    } else {
      flush()
      if (trimmed) blocks.push(<p key={blocks.length}>{inline(trimmed)}</p>)
    }
  }
  flush()
  return <div className="release-notes">{blocks}</div>
}

/** **negrita** y `código`. */
function inline(text: string): ReactNode[] {
  return text.split(/(\*\*[^*]+\*\*|`[^`]+`)/).map((part, i) => {
    if (part.startsWith('**') && part.endsWith('**')) return <strong key={i}>{part.slice(2, -2)}</strong>
    if (part.startsWith('`') && part.endsWith('`')) return <code key={i}>{part.slice(1, -1)}</code>
    return part
  })
}
