import { useEffect, useMemo, useState, type ReactNode } from 'react'
import { Link } from 'react-router'
import {
  ArrowUpRight,
  CircleCheck,
  Clock,
  Cpu,
  Gauge,
  History,
  ListOrdered,
  MemoryStick,
  MonitorPlay,
  Pause,
  Play,
  Send,
  Server,
  Trophy,
} from 'lucide-react'
import { useApi } from '../../shared/useApi'
import { formatDuration, formatSize, formatWatchTime } from '../../shared/format'
import type { ServerStatus } from '../../shared/types/ServerStatus'
import type { Viewer } from '../../shared/types/Viewer'
import type { PlaybackSummary } from '../../shared/types/PlaybackSummary'
import type { Session } from '../../shared/types/Session'
import type { ConversionSummary } from '../../shared/types/ConversionSummary'
import type { ServerHistory } from '../../shared/types/ServerHistory'
import type { HistoryPoint } from '../../shared/types/HistoryPoint'
import type { HistoryRange } from '../../shared/types/HistoryRange'
import { HistoryChart } from '../charts'
import { Alert, Badge, Card, EmptyState, Meter, MiniBars, PageHeader, StatTile, TrendBars } from '../ui'

const REFRESH_MS = 2000
/** Mediciones guardadas para las tendencias: 60 × 2 s = los últimos 2 minutos. */
const HISTORY = 60

type Point = { cpuKinora: number; cpuSystem: number; memoryKinora: number; sendRate: number }

/** Estado del servidor en vivo: CPU, memoria, transferencia y quién está mirando. */
export function StatusPage() {
  const { data: status, error, reload } = useApi<ServerStatus>('/api/admin/status')
  const { data: playback, reload: reloadPlayback } = useApi<PlaybackSummary>('/api/admin/metrics/playback')
  const { data: sessions, reload: reloadSessions } = useApi<Session[]>('/api/admin/metrics/sessions')
  const { data: conversion, reload: reloadConversion } = useApi<ConversionSummary>('/api/admin/metrics/conversion')
  const [history, setHistory] = useState<Point[]>([])

  // Los minutos vistos cambian despacio: alcanza con actualizarlos cada 30 segundos.
  useEffect(() => {
    const timer = setInterval(() => {
      reloadPlayback()
      reloadSessions()
    }, 30_000)
    return () => clearInterval(timer)
  }, [reloadPlayback, reloadSessions])

  useEffect(() => {
    const timer = setInterval(() => {
      reload()
      reloadConversion()
    }, REFRESH_MS)
    return () => clearInterval(timer)
  }, [reload, reloadConversion])

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
          value={`${sample.cpu_kinora.toLocaleString('es-AR', { minimumFractionDigits: 1, maximumFractionDigits: 1 })} %`}
          sub={`100 % = un núcleo, de ${sample.cpu_cores}`}
        >
          <TrendBars
            values={series('cpuKinora')}
            max={5}
            format={(v) => `${v.toLocaleString('es-AR', { maximumFractionDigits: v < 10 ? 1 : 0 })} %`}
            label="CPU de Kinora, últimos 2 minutos"
          />
        </StatTile>
        <StatTile icon={Gauge} label="CPU del equipo" value={`${sample.cpu_system.toFixed(0)} %`} sub="todos los núcleos">
          <TrendBars
            values={series('cpuSystem')}
            max={100}
            format={(v) => `${Math.round(v)} %`}
            label="CPU del equipo, últimos 2 minutos"
          />
        </StatTile>
        <StatTile
          icon={Server}
          label="Memoria del equipo"
          value={`${Math.round(memoryShare * 100)} %`}
          sub={`${formatSize(sample.memory_used)} de ${formatSize(sample.memory_total)}`}
        >
          <Meter value={memoryShare} warnAt={0.8} criticalAt={0.92} label="Memoria del equipo en uso" />
        </StatTile>
        <StatTile
          icon={Send}
          label="Enviando"
          value={`${formatSize(sample.send_rate)}/s`}
          sub={`${((sample.send_rate * 8) / 1_000_000).toFixed(1)} Mbps · ${formatSize(status.bytes_sent_total)} desde que arrancó`}
        >
          <TrendBars
            values={series('sendRate')}
            max={1_000_000}
            format={(v) => `${formatSize(v)}/s`}
            label="Velocidad de envío, últimos 2 minutos"
          />
        </StatTile>
        <StatTile icon={MemoryStick} label="Memoria de Kinora" value={formatSize(sample.memory_kinora)} sub="del proceso">
          <TrendBars
            values={series('memoryKinora')}
            range
            format={formatSize}
            label="Memoria de Kinora, últimos 2 minutos"
          />
        </StatTile>
        {playback && (
          <StatTile
            icon={Clock}
            label="Visto hoy"
            value={formatWatchTime(playback.today_seconds)}
            sub={`${formatWatchTime(playback.days.reduce((sum, d) => sum + d.seconds, 0))} en los últimos 7 días`}
          >
            <MiniBars
              values={playback.days.map((d) => d.seconds)}
              labels={playback.days.map((d) => `${weekday(d.day)}: ${formatWatchTime(d.seconds)}`)}
              label="Tiempo visto por día, últimos 7 días"
              format={formatWatchTime}
            />
          </StatTile>
        )}
      </div>

      {conversion && <ConversionCard summary={conversion} cpu={sample.cpu_ffmpeg} memory={sample.memory_ffmpeg} />}

      <HistoryCard />

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

      {playback && playback.top.length > 0 && (
        <div className="two-columns">
          <Card title="Lo más visto" description="últimos 30 días">
            <RankTable items={playback.top} trophy />
          </Card>
          <Card title="Por biblioteca" description="últimos 30 días">
            <RankTable items={playback.libraries} />
          </Card>
        </div>
      )}

      {sessions && sessions.length > 0 && (
        <Card title="Historial" description="las últimas visualizaciones (de más de 30 segundos)">
          <div className="table-scroll">
            <table className="table">
              <thead>
                <tr>
                  <th>Cuándo</th>
                  <th>Qué</th>
                  <th>Biblioteca</th>
                  <th>Equipo</th>
                  <th>Tramo</th>
                  <th className="num">Visto</th>
                </tr>
              </thead>
              <tbody>
                {sessions.map((session) => (
                  <tr key={session.id}>
                    <td className="secondary">{when(session.started_at)}</td>
                    <td>
                      <span className="row">
                        <strong>{session.title}</strong>
                        {session.completed && <CircleCheck size={15} color="var(--good)" aria-label="terminado" />}
                      </span>
                    </td>
                    <td className="secondary">{session.library_name ?? '—'}</td>
                    <td className="mono">{session.client}</td>
                    <td className="secondary">
                      {formatDuration(session.start_position)} → {formatDuration(session.end_position)}
                    </td>
                    <td className="num">{formatWatchTime(session.seconds_watched)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </Card>
      )}
      {sessions && sessions.length === 0 && (
        <Card title="Historial">
          <EmptyState icon={History} title="Todavía no hay visualizaciones registradas" />
        </Card>
      )}

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

function RankTable({ items, trophy = false }: { items: { title: string; seconds: number }[]; trophy?: boolean }) {
  const top = items[0]?.seconds ?? 0
  return (
    <table className="table">
      <tbody>
        {items.map((item, i) => (
          <tr key={item.title}>
            <td style={{ width: 36 }} className="muted">
              {trophy && i === 0 ? <Trophy size={16} /> : i + 1}
            </td>
            <td>
              <strong>{item.title}</strong>
            </td>
            <td>
              <div className="size-cell">
                <span>{formatWatchTime(item.seconds)}</span>
                <Meter value={top > 0 ? item.seconds / top : 0} small label={item.title} />
              </div>
            </td>
          </tr>
        ))}
      </tbody>
    </table>
  )
}

/** "hoy 14:05", "ayer 21:30" o "lun 12 21:30". */
function when(seconds: number) {
  const date = new Date(seconds * 1000)
  const hour = date.toLocaleTimeString('es-AR', { hour12: false, hour: '2-digit', minute: '2-digit' })
  const today = new Date()
  const yesterday = new Date(Date.now() - 86_400_000)
  if (date.toDateString() === today.toDateString()) return `hoy ${hour}`
  if (date.toDateString() === yesterday.toDateString()) return `ayer ${hour}`
  return `${date.toLocaleDateString('es-AR', { weekday: 'short', day: 'numeric', month: 'short' })} ${hour}`
}

/** "2026-10-12" → "lun 12" */
function weekday(day: string) {
  const date = new Date(`${day}T12:00:00`)
  return date.toLocaleDateString('es-AR', { weekday: 'short', day: 'numeric' })
}

function time(seconds: number) {
  return new Date(seconds * 1000).toLocaleTimeString('es-AR', { hour12: false, hour: '2-digit', minute: '2-digit' })
}

/** La cola y lo que rindió: cuánto espacio se ahorró y a qué velocidad convierte este equipo. */
function ConversionCard({
  summary,
  cpu,
  memory,
}: {
  summary: ConversionSummary
  cpu: number | null
  memory: number | null
}) {
  const queued = summary.pending + summary.running
  return (
    <Card
      title="Conversión"
      description="la cola de importación"
      actions={
        <Link to="/jobs" className="btn btn-ghost btn-sm">
          <ListOrdered /> Ver la cola
        </Link>
      }
    >
      <div className="card-body stat-row">
        <Stat label="En la cola" value={queued === 0 ? 'Nada' : String(queued)}>
          {summary.running > 0 && cpu !== null
            ? `ffmpeg: ${cpu.toFixed(0)} % CPU · ${formatSize(memory ?? 0)}`
            : summary.running > 0
              ? 'procesando'
              : summary.pending > 0
                ? 'detenida: iniciala desde Cola'
                : 'nada pendiente'}
        </Stat>
        <Stat label="Espacio ahorrado" value={formatSize(summary.saved_bytes)}>
          convirtiendo a HEVC, desde siempre
        </Stat>
        <Stat label="Velocidad" value={summary.speed === null ? '—' : `${summary.speed.toFixed(1)}× tiempo real`}>
          {summary.speed === null
            ? 'todavía no hubo conversiones'
            : `un episodio de 24 min tarda ${Math.round(24 / summary.speed)} min`}
        </Stat>
        <Stat label="Últimos 30 días" value={`${summary.done} listos`}>
          {summary.failed === 0 ? 'ninguno falló' : `${summary.failed} fallaron`}
        </Stat>
      </div>
    </Card>
  )
}

function Stat({ label, value, children }: { label: string; value: string; children: ReactNode }) {
  return (
    <div className="stat">
      <div className="tile-label">{label}</div>
      <div className="stat-value">{value}</div>
      <div className="tile-sub">{children}</div>
    </div>
  )
}

const RANGES: [HistoryRange, string][] = [
  ['day', '24 horas'],
  ['week', '7 días'],
  ['month', '30 días'],
]

/** CPU, memoria y envío del último día, semana o mes (una medición por minuto, guardada 30 días). */
function HistoryCard() {
  const [range, setRange] = useState<HistoryRange>('day')
  const { data: history, reload } = useApi<ServerHistory>(`/api/admin/metrics/server?range=${range}`)
  const [hover, setHover] = useState<number | null>(null)

  // Los puntos nuevos llegan de a uno por minuto.
  useEffect(() => {
    const timer = setInterval(reload, 60_000)
    return () => clearInterval(timer)
  }, [reload])

  // Un lugar por período, con o sin datos: los que no tienen son ratos con el servidor apagado.
  const slots = useMemoSlots(history)
  const axisLabel = (t: number, i: number) => {
    if (i === 0) return ''
    const date = new Date(t * 1000)
    const previous = new Date(slots[i - 1].t * 1000)
    if (range === 'day') {
      return Math.floor(date.getHours() / 6) !== Math.floor(previous.getHours() / 6) ? clock(date) : ''
    }
    const newDay = date.getDate() !== previous.getDate()
    if (range === 'week') return newDay ? weekdayShort(date) : ''
    return newDay && date.getDay() === 1 ? dayMonth(date) : ''
  }
  const hovered = hover !== null ? slots[hover] : null
  const empty = history && history.points.length === 0

  return (
    <Card
      title="Historial del servidor"
      description={
        hovered
          ? `${fullDate(new Date(hovered.t * 1000))}${hovered.point ? '' : ' · apagado'}`
          : 'promedios; los cortes son ratos con el servidor apagado'
      }
      actions={
        <div className="segmented" role="group" aria-label="Período">
          {RANGES.map(([value, label]) => (
            <button key={value} type="button" aria-pressed={range === value} onClick={() => setRange(value)}>
              {label}
            </button>
          ))}
        </div>
      }
    >
      {empty ? (
        <EmptyState icon={History} title="Todavía no hay historial">
          Se guarda una medición por minuto desde que se actualizó Kinora.
        </EmptyState>
      ) : (
        <div className="card-body history-charts">
          <HistoryChart
            title="CPU"
            slots={slots}
            series={[
              { label: 'Kinora (con conversiones)', tone: 'ink', value: (p: HistoryPoint) => p.cpu_kinora + p.cpu_ffmpeg },
              { label: 'Todo el equipo', tone: 'context', value: (p: HistoryPoint) => p.cpu_system },
            ]}
            max={100}
            format={percent}
            axisLabel={axisLabel}
            hover={hover}
            onHover={setHover}
          />
          <HistoryChart
            title="Memoria del equipo"
            slots={slots}
            series={[{ label: 'En uso', tone: 'ink', value: (p: HistoryPoint) => p.memory_share * 100 }]}
            max={100}
            format={percent}
            axisLabel={axisLabel}
            hover={hover}
            onHover={setHover}
          />
          <HistoryChart
            title="Envío de video"
            slots={slots}
            series={[{ label: 'Promedio', tone: 'ink', value: (p: HistoryPoint) => p.send_mbps }]}
            format={(v) => `${v < 10 && v > 0 ? v.toFixed(1) : Math.round(v)} Mbps`}
            axisLabel={axisLabel}
            hover={hover}
            onHover={setHover}
          />
        </div>
      )}
    </Card>
  )
}

function useMemoSlots(history: ServerHistory | undefined) {
  return useMemo(() => {
    if (!history) return []
    const byTime = new Map(history.points.map((p) => [p.t, p]))
    const slots: { t: number; point: HistoryPoint | null }[] = []
    for (let t = history.from; t < history.to; t += history.step) slots.push({ t, point: byTime.get(t) ?? null })
    return slots
  }, [history])
}

function percent(value: number) {
  return value > 0 && value < 1 ? `${value.toFixed(1).replace('.', ',')} %` : `${Math.round(value)} %`
}

function clock(date: Date) {
  return date.toLocaleTimeString('es-AR', { hour: '2-digit', minute: '2-digit', hour12: false })
}

function weekdayShort(date: Date) {
  return date.toLocaleDateString('es-AR', { weekday: 'short', day: 'numeric' })
}

function dayMonth(date: Date) {
  return date.toLocaleDateString('es-AR', { day: 'numeric', month: 'short' })
}

function fullDate(date: Date) {
  return date.toLocaleString('es-AR', {
    weekday: 'long',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
    hour12: false,
  })
}
