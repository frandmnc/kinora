import { useEffect, useState } from 'react'
import { Link } from 'react-router'
import { CircleCheck, CircleX, Clock, ListOrdered, LoaderCircle, Ban } from 'lucide-react'
import { api } from '../../shared/api'
import { useApi } from '../../shared/useApi'
import { formatSize } from '../../shared/format'
import type { JobView } from '../../shared/types/JobView'
import type { JobState } from '../../shared/types/JobState'
import { Alert, Badge, Card, EmptyState, Meter, PageHeader } from '../ui'
import { OPERATION_LABELS } from '../labels'

/** La cola de importación: lo que espera, lo que corre y lo último que terminó. */
export function JobsPage() {
  const { data: jobs, error, reload } = useApi<JobView[]>('/api/admin/jobs')
  const [actionError, setActionError] = useState<string>()
  const active = jobs?.some((job) => job.state === 'pending' || job.state === 'running') ?? false

  // Mientras hay algo en marcha se actualiza cada segundo; si no, cada 10.
  useEffect(() => {
    const timer = setInterval(reload, active ? 1000 : 10_000)
    return () => clearInterval(timer)
  }, [reload, active])

  async function cancel(job: JobView) {
    if (!confirm(`¿Cancelar "${job.title}"? El archivo vuelve a la bandeja de entrada.`)) return
    setActionError(undefined)
    try {
      await api(`/api/admin/jobs/${job.id}/cancel`, { method: 'POST' })
      reload()
    } catch (err) {
      setActionError((err as Error).message)
    }
  }

  const current = jobs?.filter((job) => job.state === 'pending' || job.state === 'running') ?? []
  const finished = jobs?.filter((job) => job.state !== 'pending' && job.state !== 'running') ?? []

  return (
    <>
      <PageHeader
        title="Cola"
        description="Los archivos que hay que procesar antes de entrar a la biblioteca. Se hace uno por vez; si el servidor se reinicia, el que estaba en curso vuelve a empezar."
      />
      {error && <Alert>No se pudo cargar la cola: {error}</Alert>}
      {actionError && <Alert>{actionError}</Alert>}

      <Card title="En curso" description={current.length > 0 ? `${current.length} trabajo(s)` : undefined}>
        {jobs && current.length === 0 && (
          <EmptyState icon={ListOrdered} title="No hay nada en la cola">
            Los MKV que importes desde <Link to="/import">Importar</Link> pasan por acá.
          </EmptyState>
        )}
        {current.map((job) => (
          <JobRow key={job.id} job={job} onCancel={() => cancel(job)} />
        ))}
      </Card>

      {finished.length > 0 && (
        <Card title="Terminados" description="los últimos 20">
          {finished.map((job) => (
            <JobRow key={job.id} job={job} />
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

function JobRow({ job, onCancel }: { job: JobView; onCancel?: () => void }) {
  const state = STATES[job.state]
  const Icon = STATE_ICONS[job.state]
  const now = Date.now() / 1000
  const elapsed = job.started_at ? (job.finished_at ?? now) - job.started_at : 0
  // Tiempo restante estimado: lo que tardó hasta acá, proporcional a lo que falta.
  const remaining = job.state === 'running' && job.progress > 0.02 ? (elapsed / job.progress) * (1 - job.progress) : null

  return (
    <div className="job-row">
      <Icon className={`icon ${job.state === 'running' ? 'spin' : ''}`} />
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
              {Math.round(job.progress * 100)}%{remaining !== null && ` · faltan ${formatSeconds(remaining)}`}
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
        {job.state === 'done' && `en ${formatSeconds(elapsed)}`}
        {onCancel && (
          <button className="btn btn-ghost btn-sm btn-danger" onClick={onCancel}>
            Cancelar
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
