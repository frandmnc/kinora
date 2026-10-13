import { useEffect, useState } from 'react'
import { Ban, CircleCheck, CircleX, Clock, ListOrdered, LoaderCircle, Pause, Play } from 'lucide-react'
import { api } from '../../shared/api'
import { useApi } from '../../shared/useApi'
import { formatSize } from '../../shared/format'
import type { JobView } from '../../shared/types/JobView'
import type { QueueView } from '../../shared/types/QueueView'
import type { JobState } from '../../shared/types/JobState'
import { Alert, Badge, Card, EmptyState, Meter, PageHeader } from '../ui'
import { OPERATION_LABELS } from '../labels'

/** La cola de importación: lo que espera, lo que corre y lo último que terminó. */
export function JobsPage() {
  const { data: queue, error, reload } = useApi<QueueView>('/api/admin/jobs')
  const jobs = queue?.jobs
  const [actionError, setActionError] = useState<string>()
  const active = jobs?.some((job) => job.state === 'running') ?? false

  // Mientras hay algo en marcha se actualiza cada segundo; si no, cada 10.
  useEffect(() => {
    const timer = setInterval(reload, active ? 1000 : 10_000)
    return () => clearInterval(timer)
  }, [reload, active])

  async function setStarted(started: boolean) {
    setActionError(undefined)
    try {
      await api(`/api/admin/jobs/${started ? 'start' : 'pause'}`, { method: 'POST' })
      reload()
    } catch (err) {
      setActionError((err as Error).message)
    }
  }

  async function cancel(job: JobView) {
    const what = job.state === 'pending' ? 'Quitar de la cola' : 'Cancelar'
    if (!confirm(`¿${what} "${job.title}"? El archivo vuelve a la bandeja de entrada.`)) return
    setActionError(undefined)
    try {
      await api(`/api/admin/jobs/${job.id}/cancel`, { method: 'POST' })
      reload()
    } catch (err) {
      setActionError((err as Error).message)
    }
  }

  // El que se está procesando va aparte; los que esperan, en el orden en que se van a hacer.
  const processing = jobs?.find((job) => job.state === 'running')
  const waiting = (jobs?.filter((job) => job.state === 'pending') ?? []).sort((a, b) => a.id - b.id)
  const pending = waiting.length
  const running = processing !== undefined
  const started = queue?.started ?? false
  const finished = jobs?.filter((job) => job.state !== 'pending' && job.state !== 'running') ?? []

  return (
    <>
      <PageHeader
        title="Cola"
        description="Los archivos que hay que procesar antes de entrar a la biblioteca. Importar los agrega sin empezarlos: la cola arranca cuando la iniciás, hace uno por vez y se detiene sola al terminar. Si el servidor se reinicia, sigue como estaba."
        actions={
          started ? (
            <>
              <Badge tone="good" icon={Play}>
                En marcha
              </Badge>
              <button
                className="btn"
                onClick={() => setStarted(false)}
                title="Termina el trabajo en curso y no empieza el siguiente"
              >
                <Pause /> Pausar
              </button>
            </>
          ) : (
            <button className="btn btn-primary" disabled={pending === 0} onClick={() => setStarted(true)}>
              <Play /> Iniciar cola{pending > 0 && ` (${pending})`}
            </button>
          )
        }
      />
      {!started && running && (
        <Alert tone="info">Pausada: termina el trabajo en curso y después se detiene.</Alert>
      )}
      {!started && !running && pending > 0 && (
        <Alert tone="info">
          La cola está detenida: {pending} trabajo{pending === 1 ? '' : 's'} esperando. Tocá "Iniciar cola" cuando
          quieras empezar.
        </Alert>
      )}
      {error && <Alert>No se pudo cargar la cola: {error}</Alert>}
      {actionError && <Alert>{actionError}</Alert>}

      <Card title="Procesando">
        {processing ? (
          <JobRow job={processing} started={started} onCancel={() => cancel(processing)} />
        ) : (
          jobs && (
            <EmptyState icon={ListOrdered} title="Nada en proceso">
              {pending > 0
                ? 'Los trabajos en espera empiezan cuando inicies la cola.'
                : 'Los archivos que necesitan remux o conversión, al importarlos desde Importar, esperan acá.'}
            </EmptyState>
          )
        )}
      </Card>

      {waiting.length > 0 && (
        <Card title="En espera" description={`${waiting.length} trabajo(s), en el orden en que se van a hacer`}>
          {waiting.map((job, i) => (
            <JobRow key={job.id} job={job} started={started} position={i + 1} onCancel={() => cancel(job)} />
          ))}
        </Card>
      )}

      {finished.length > 0 && (
        <Card title="Terminados" description="los últimos 20">
          {finished.map((job) => (
            <JobRow key={job.id} job={job} started={started} />
          ))}
        </Card>
      )}
    </>
  )
}

const STATES: Record<JobState, { label: string; tone?: 'good' | 'warning' | 'critical' }> = {
  pending: { label: 'Esperando' },
  running: { label: 'En curso' },
  done: { label: 'Listo', tone: 'good' },
  failed: { label: 'Falló', tone: 'critical' },
  cancelled: { label: 'Cancelado', tone: 'warning' },
}

const STATE_ICONS = {
  pending: Clock,
  running: LoaderCircle,
  done: CircleCheck,
  failed: CircleX,
  cancelled: Ban,
}

function JobRow({
  job,
  started,
  position,
  onCancel,
}: {
  job: JobView
  started: boolean
  /** Lugar en la fila de espera (1 = el próximo). */
  position?: number
  onCancel?: () => void
}) {
  const state = job.state === 'pending' && !started ? { label: 'En espera' } : STATES[job.state]
  const Icon = STATE_ICONS[job.state]
  const now = Date.now() / 1000
  const elapsed = job.started_at ? (job.finished_at ?? now) - job.started_at : 0
  // Tiempo restante estimado: lo que tardó hasta acá, proporcional a lo que falta.
  const remaining = job.state === 'running' && job.progress > 0.02 ? (elapsed / job.progress) * (1 - job.progress) : null
  // Velocidad: cuántos segundos de video procesa por segundo (2× = la mitad de lo que dura).
  const done = job.state === 'done' ? 1 : job.progress
  const speed = job.duration && elapsed > 2 && done > 0.02 ? (job.duration * done) / elapsed : null

  return (
    <div className="job-row">
      {position !== undefined ? (
        <span className="job-position" aria-label={`Lugar ${position} en la fila`}>
          {position}
        </span>
      ) : (
        <Icon className={`icon ${job.state === 'running' ? 'spin' : ''}`} />
      )}
      <div className="job-main">
        <div className="row">
          <strong className="truncate">{job.title}</strong>
          <Badge tone={state.tone}>{state.label}</Badge>
          <span className="small muted">{OPERATION_LABELS[job.operation]}</span>
        </div>
        <div className="small muted mono truncate" title={job.source_name}>
          {job.source_name} · {formatSize(job.source_size)}
          {job.result_size !== null && ` → ${formatSize(job.result_size)}`}
        </div>
        {job.state === 'running' && (
          <div className="row job-progress">
            <Meter value={job.progress} label={`Avance de ${job.title}`} />
            <span className="small muted">
              {Math.round(job.progress * 100)}%{speed !== null && ` · ${speed.toFixed(1)}×`}
              {remaining !== null && ` · faltan ${formatSeconds(remaining)}`}
            </span>
          </div>
        )}
        {job.error && (
          <div className="small" style={{ color: 'var(--critical-text)' }}>
            {job.error}. El archivo volvió a la bandeja de entrada.
          </div>
        )}
        {job.state === 'cancelled' && <div className="small muted">El archivo volvió a la bandeja de entrada.</div>}
      </div>
      <div className="job-side small muted">
        {job.state === 'done' && `en ${formatSeconds(elapsed)}${speed !== null ? ` · ${speed.toFixed(1)}×` : ''}`}
        {onCancel && (
          <button className="btn btn-ghost btn-sm btn-danger" onClick={onCancel}>
            {job.state === 'pending' ? 'Quitar' : 'Cancelar'}
          </button>
        )}
      </div>
    </div>
  )
}

function formatSeconds(seconds: number) {
  if (seconds < 60) return `${Math.max(1, Math.round(seconds))} s`
  const minutes = Math.round(seconds / 60)
  return minutes < 60 ? `${minutes} min` : `${Math.floor(minutes / 60)} h ${minutes % 60} min`
}
