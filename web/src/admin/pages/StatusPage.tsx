import { useEffect, useState } from 'react'
import { ArrowUpRight, Cpu, Gauge, MemoryStick, MonitorPlay, Pause, Play, Send, Server } from 'lucide-react'
import { useApi } from '../../shared/useApi'
import { formatDuration, formatSize } from '../../shared/format'
import type { ServerStatus } from '../../shared/types/ServerStatus'
import type { Viewer } from '../../shared/types/Viewer'
import { Alert, Badge, Card, EmptyState, Meter, PageHeader, Sparkline, StatTile } from '../ui'

const REFRESH_MS = 2000
/** Mediciones guardadas para las tendencias: 60 × 2 s = los últimos 2 minutos. */
const HISTORY = 60

type Point = { cpuKinora: number; cpuSystem: number; memoryKinora: number; sendRate: number }

/** Estado del servidor en vivo: CPU, memoria, transferencia y quién está mirando. */
export function StatusPage() {
  const { data: status, error, reload } = useApi<ServerStatus>('/api/admin/status')
  const [history, setHistory] = useState<Point[]>([])

  useEffect(() => {
    const timer = setInterval(reload, REFRESH_MS)
    return () => clearInterval(timer)
  }, [reload])

  // Cada respuesta nueva suma un punto a las tendencias (la historia vive solo en esta pantalla).
  useEffect(() => {
    if (!status) return
    const { sample } = status
    setHistory((points) =>
      [
        ...points,
        {
          cpuKinora: sample.cpu_kinora,
          cpuSystem: sample.cpu_system,
          memoryKinora: sample.memory_kinora,
          sendRate: sample.send_rate,
        },
      ].slice(-HISTORY),
    )
  }, [status])

  if (error && !status) return <Alert>No hay conexión con el servidor: {error}</Alert>
  if (!status) return <p className="muted">Cargando…</p>

  const { sample } = status
  const series = (key: keyof Point) => history.map((point) => point[key])
  const memoryShare = sample.memory_total > 0 ? sample.memory_used / sample.memory_total : 0

  return (
    <>
      <PageHeader
        title="Estado"
        description={`Encendido hace ${formatDuration(status.uptime_secs)}. Se actualiza cada ${REFRESH_MS / 1000} segundos; las tendencias muestran los últimos 2 minutos.`}
      />
      {error && <Alert tone="warning">Se perdió la conexión con el servidor: {error}</Alert>}

      <div className="tiles">
        <StatTile
          icon={Cpu}
          label="CPU de Kinora"
          value={`${sample.cpu_kinora.toFixed(1)} %`}
          sub={`100 % = un núcleo, de ${sample.cpu_cores}`}
        >
          <Sparkline values={series('cpuKinora')} max={5} label="CPU de Kinora, últimos 2 minutos" />
        </StatTile>
        <StatTile icon={Gauge} label="CPU del equipo" value={`${sample.cpu_system.toFixed(0)} %`} sub="todos los núcleos">
          <Sparkline values={series('cpuSystem')} max={100} label="CPU del equipo, últimos 2 minutos" />
        </StatTile>
        <StatTile
          icon={Send}
          label="Enviando"
          value={`${formatSize(sample.send_rate)}/s`}
          sub={`${((sample.send_rate * 8) / 1_000_000).toFixed(1)} Mbps · ${formatSize(status.bytes_sent_total)} desde que arrancó`}
        >
          <Sparkline values={series('sendRate')} label="Velocidad de envío, últimos 2 minutos" />
        </StatTile>
        <StatTile icon={MemoryStick} label="Memoria de Kinora" value={formatSize(sample.memory_kinora)} sub="del proceso">
          <Sparkline values={series('memoryKinora')} range label="Memoria de Kinora, últimos 2 minutos" />
        </StatTile>
        <StatTile
          icon={Server}
          label="Memoria del equipo"
          value={`${Math.round(memoryShare * 100)} %`}
          sub={`${formatSize(sample.memory_used)} de ${formatSize(sample.memory_total)}`}
        >
          <Meter value={memoryShare} warnAt={0.8} criticalAt={0.92} label="Memoria del equipo en uso" />
        </StatTile>
      </div>

      <Card
        title="Viendo ahora"
        description="según lo que informa cada reproductor cada 10 segundos"
        actions={<Badge tone={status.viewers.length > 0 ? 'accent' : undefined}>{status.viewers.length}</Badge>}
      >
        {status.viewers.length === 0 ? (
          <EmptyState icon={MonitorPlay} title="Nadie está mirando" />
        ) : (
          status.viewers.map((viewer) => <ViewerRow key={`${viewer.client}-${viewer.content_id}`} viewer={viewer} />)
        )}
      </Card>

      <Card title="Transferencias en curso" description="descargas de video que el servidor está enviando ahora">
        {status.transfers.length === 0 ? (
          <EmptyState icon={ArrowUpRight} title="Ninguna en este momento">
            El navegador descarga el video por tramos de varios minutos y después hace pausas: es normal que no
            haya transferencias aunque alguien esté mirando.
          </EmptyState>
        ) : (
          <table className="table">
            <thead>
              <tr>
                <th>Equipo</th>
                <th>Archivo</th>
                <th>Desde</th>
                <th className="num">Enviado</th>
              </tr>
            </thead>
            <tbody>
              {status.transfers.map((transfer) => (
                <tr key={`${transfer.client}-${transfer.file_id}`}>
                  <td className="mono">{transfer.client}</td>
                  <td className="mono">{transfer.file_name}</td>
                  <td className="secondary">{time(transfer.started_at)}</td>
                  <td className="num">{formatSize(transfer.bytes_sent)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </Card>
    </>
  )
}

function ViewerRow({ viewer }: { viewer: Viewer }) {
  const share = viewer.duration ? viewer.position / viewer.duration : 0
  return (
    <div className="viewer">
      <div className="viewer-icon">
        <MonitorPlay />
      </div>
      <div className="grow">
        <div className="row">
          <span className="viewer-title truncate">{viewer.title}</span>
          {viewer.playing ? (
            <Badge tone="accent" icon={Play}>
              Reproduciendo
            </Badge>
          ) : (
            <Badge icon={Pause}>En pausa</Badge>
          )}
        </div>
        <div className="viewer-progress">
          <span className="mono">{viewer.client}</span>
          <Meter value={share} label="Avance" />
          <span>
            {formatDuration(viewer.position)} / {formatDuration(viewer.duration)}
          </span>
          <span>· desde las {time(viewer.started_at)}</span>
        </div>
      </div>
    </div>
  )
}

function time(seconds: number) {
  return new Date(seconds * 1000).toLocaleTimeString('es-AR', { hour12: false, hour: '2-digit', minute: '2-digit' })
}
