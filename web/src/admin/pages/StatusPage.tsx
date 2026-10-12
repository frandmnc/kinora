import { useEffect } from 'react'
import { useApi } from '../../shared/useApi'
import { formatDuration, formatSize } from '../../shared/format'
import type { ServerStatus } from '../../shared/types/ServerStatus'

const REFRESH_MS = 2000

/** Estado del servidor en vivo: CPU, memoria, transferencia y quién está mirando. */
export function StatusPage() {
  const { data: status, error, reload } = useApi<ServerStatus>('/api/admin/status')

  useEffect(() => {
    const timer = setInterval(reload, REFRESH_MS)
    return () => clearInterval(timer)
  }, [reload])

  if (error && !status) return <p className="error">{error}</p>
  if (!status) return <p className="muted">Cargando…</p>

  const { sample } = status
  return (
    <>
      <div className="page-header">
        <h1>Estado</h1>
        <span className="muted">
          Encendido hace {formatDuration(status.uptime_secs)} · se actualiza cada {REFRESH_MS / 1000} s
        </span>
        {error && <span className="error">Sin conexión: {error}</span>}
      </div>

      <div className="stats">
        <Stat
          label="CPU de Kinora"
          value={`${sample.cpu_kinora.toFixed(1)} %`}
          detail={`100 % = un núcleo de ${sample.cpu_cores}`}
        />
        <Stat label="CPU total" value={`${sample.cpu_system.toFixed(0)} %`} detail="todo el equipo" />
        <Stat label="Memoria de Kinora" value={formatSize(sample.memory_kinora)} />
        <Stat
          label="Memoria total"
          value={formatSize(sample.memory_used)}
          detail={`de ${formatSize(sample.memory_total)}`}
        />
        <Stat
          label="Enviando"
          value={`${formatSize(sample.send_rate)}/s`}
          detail={`${((sample.send_rate * 8) / 1_000_000).toFixed(1)} Mbps`}
        />
        <Stat label="Enviado desde el inicio" value={formatSize(status.bytes_sent_total)} />
      </div>

      <h2>Viendo ahora</h2>
      {status.viewers.length === 0 ? (
        <p className="muted">Nadie está mirando.</p>
      ) : (
        <table className="table">
          <thead>
            <tr>
              <th>Equipo</th>
              <th>Qué</th>
              <th>Estado</th>
              <th>Posición</th>
              <th>Desde</th>
            </tr>
          </thead>
          <tbody>
            {status.viewers.map((viewer) => (
              <tr key={`${viewer.client}-${viewer.content_id}`}>
                <td className="mono">{viewer.client}</td>
                <td>{viewer.title}</td>
                <td>{viewer.playing ? '▶ Reproduciendo' : '⏸ En pausa'}</td>
                <td>
                  <div className="position">
                    <span>
                      {formatDuration(viewer.position)} / {formatDuration(viewer.duration)}
                    </span>
                    {viewer.duration && (
                      <span className="bar">
                        <span style={{ width: `${(viewer.position / viewer.duration) * 100}%` }} />
                      </span>
                    )}
                  </div>
                </td>
                <td>{time(viewer.started_at)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
      <p className="muted small">Según lo que informa cada reproductor cada 10 segundos.</p>

      <h2>Transferencias en curso</h2>
      {status.transfers.length === 0 ? (
        <p className="muted">Ninguna en este momento.</p>
      ) : (
        <table className="table">
          <thead>
            <tr>
              <th>Equipo</th>
              <th>Archivo</th>
              <th>Desde</th>
              <th>Enviado</th>
            </tr>
          </thead>
          <tbody>
            {status.transfers.map((transfer) => (
              <tr key={`${transfer.client}-${transfer.file_id}`}>
                <td className="mono">{transfer.client}</td>
                <td className="mono">{transfer.file_name}</td>
                <td>{time(transfer.started_at)}</td>
                <td>{formatSize(transfer.bytes_sent)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
      <p className="muted small">
        El navegador descarga el video por tramos (de a varios minutos) y después hace pausas: por eso las
        transferencias aparecen y desaparecen aunque alguien siga mirando.
      </p>
    </>
  )
}

function time(seconds: number) {
  return new Date(seconds * 1000).toLocaleTimeString('es-AR', { hour12: false })
}

function Stat({ label, value, detail }: { label: string; value: string; detail?: string }) {
  return (
    <div className="stat">
      <div className="stat-label">{label}</div>
      <div className="stat-value">{value}</div>
      {detail && <div className="stat-detail">{detail}</div>}
    </div>
  )
}
