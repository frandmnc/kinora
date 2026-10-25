import { useEffect, useState, type ReactNode } from 'react'
import { ChevronDown, ChevronUp, Download, LoaderCircle, RefreshCw } from 'lucide-react'
import { api } from '../shared/api'
import { useApi } from '../shared/useApi'
import type { Health } from '../shared/types/Health'
import type { InstallUpdate } from '../shared/types/InstallUpdate'
import type { UpdateStatus } from '../shared/types/UpdateStatus'
import { Card } from './ui'

/** Cuánto esperar a que Kinora vuelva con la versión nueva antes de avisar que tarda. */
const RESTART_TIMEOUT_MS = 3 * 60_000

/**
 * La versión de Kinora, en una fila: la instalada y si hay una más nueva en GitHub. Si la hay,
 * sus notas a mano y el botón para instalarla: el servidor la baja, la verifica y se reinicia
 * con ella (lo hace la app que lo supervisa, en la barra de menú de la Mac o la bandeja de
 * Windows).
 */
export function UpdatesCard() {
  const { data: status, error, loading, reload } = useApi<UpdateStatus>('/api/admin/updates')
  const [showNotes, setShowNotes] = useState(false)
  const [installing, setInstalling] = useState<string>()
  const [installError, setInstallError] = useState<string>()
  const [slow, setSlow] = useState(false)

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

  const latest = status?.update_available ? status.latest : null

  let message: ReactNode
  let actions: ReactNode = null
  if (installing) {
    message = (
      <span className="row">
        <LoaderCircle className="spin" /> Instalando {installing}: Kinora se reinicia y esta página se recarga sola.
        {slow && ' Está tardando: mirá el ícono de Kinora en la compu donde corre.'}
      </span>
    )
  } else if (error) {
    message = <span className="update-error">No se pudo consultar GitHub: {error}</span>
  } else if (!status) {
    message = <span className="muted">Buscando versiones…</span>
  } else if (latest) {
    message = (
      <span>
        Hay una versión nueva, <strong>{latest.version}</strong>
        <span className="muted">, del {formatDate(latest.published)}</span>
      </span>
    )
    actions = (
      <>
        <button className="btn btn-sm btn-ghost" onClick={() => setShowNotes(!showNotes)}>
          {showNotes ? <ChevronUp /> : <ChevronDown />} Qué trae
        </button>
        {!status.cannot_install && (
          <button className="btn btn-sm btn-primary" onClick={() => install(latest.version)}>
            <Download /> Actualizar
          </button>
        )}
      </>
    )
  } else {
    message = <span className="muted">Es la última versión.</span>
  }

  return (
    <Card>
      <div className="card-body update-row">
        <span className="update-version">Kinora {status?.current ?? ''}</span>
        <span className="grow">{message}</span>
        {actions}
        {!installing && !latest && (
          <button className="btn btn-sm btn-ghost" onClick={reload} disabled={loading} title="Buscar una versión nueva">
            <RefreshCw /> Buscar
          </button>
        )}
      </div>
      {latest && showNotes && !installing && (
        <div className="card-body update-details">
          <ReleaseNotes text={latest.notes} />
          {status?.cannot_install && <p className="small muted">{status.cannot_install}.</p>}
        </div>
      )}
      {installError && <div className="card-body update-error">{installError}</div>}
    </Card>
  )
}

/** "2026-10-24" → "24 de octubre". Con el año, si no es este. */
function formatDate(day: string) {
  const [year, month, date] = day.split('-').map(Number)
  const thisYear = new Date().getFullYear() === year
  return new Date(year, month - 1, date).toLocaleDateString('es-AR', {
    day: 'numeric',
    month: 'long',
    year: thisYear ? undefined : 'numeric',
  })
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
