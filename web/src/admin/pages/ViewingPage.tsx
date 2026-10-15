import { useState } from 'react'
import { CircleCheck, Clock, Eye, Popcorn, Trophy } from 'lucide-react'
import { useApi } from '../../shared/useApi'
import { formatWatchTime } from '../../shared/format'
import type { ViewingRange } from '../../shared/types/ViewingRange'
import type { ViewingStats } from '../../shared/types/ViewingStats'
import type { TitleTotal } from '../../shared/types/TitleTotal'
import { ColumnChart } from '../charts'
import { Alert, Card, EmptyState, Meter, PageHeader, StatTile, niceMax } from '../ui'

const RANGES: [ViewingRange, string][] = [
  ['week', '7 días'],
  ['month', '30 días'],
  ['year', '12 meses'],
  ['all', 'Todo'],
]

/** Qué se mira y cuánto: totales, lo más visto y el reparto por biblioteca y por perfil. */
export function ViewingPage() {
  const [range, setRange] = useState<ViewingRange>('month')
  const { data: stats, error } = useApi<ViewingStats>(`/api/admin/metrics/viewing?range=${range}`)

  return (
    <>
      <PageHeader
        title="Visualizaciones"
        description="Qué se mira en el Player y cuánto. Lo que pasa ahora en el servidor está en Estado."
        actions={
          <div className="segmented" role="group" aria-label="Período">
            {RANGES.map(([value, label]) => (
              <button key={value} type="button" aria-pressed={range === value} onClick={() => setRange(value)}>
                {label}
              </button>
            ))}
          </div>
        }
      />
      {error && <Alert>No se pudieron cargar las métricas: {error}</Alert>}
      {!stats ? (
        !error && <p className="muted">Cargando…</p>
      ) : (
        <Stats stats={stats} range={range} />
      )}
    </>
  )
}

function Stats({ stats, range }: { stats: ViewingStats; range: ViewingRange }) {
  const since = range === 'all' ? `desde el ${longDate(stats.from)}` : `en los últimos ${RANGES.find(([r]) => r === range)?.[1]}`
  const average = stats.total_seconds / Math.max(1, stats.buckets.length)
  const unit = stats.monthly ? 'mes' : 'día'

  if (stats.total_seconds === 0 && stats.sessions === 0) {
    return (
      <Card>
        <EmptyState icon={Popcorn} title={`No se vio nada ${since}`} />
      </Card>
    )
  }

  return (
    <>
      <div className="tiles">
        <StatTile icon={Clock} label="Tiempo visto" value={formatWatchTime(stats.total_seconds)} sub={since} />
        <StatTile
          icon={Eye}
          label="Visualizaciones"
          value={stats.sessions.toLocaleString('es-AR')}
          sub="de al menos 30 segundos"
        />
        <StatTile
          icon={CircleCheck}
          label="Vistas hasta el final"
          value={stats.completed.toLocaleString('es-AR')}
          sub={
            stats.sessions > 0
              ? `${Math.round((stats.completed / stats.sessions) * 100)} % de las visualizaciones`
              : 'ninguna visualización'
          }
        />
      </div>

      <Card>
        <div className="card-body">
          <ColumnChart
            title={`Tiempo visto por ${unit}`}
            summary={`${formatWatchTime(average)} por ${unit} en promedio`}
            columns={stats.buckets.map((bucket, i) => ({
              name: stats.monthly ? monthName(bucket.day) : dayName(bucket.day),
              label: axisLabel(stats, i),
              value: bucket.seconds,
            }))}
            format={formatWatchTime}
            scale={niceDuration}
          />
        </div>
      </Card>

      <div className="two-columns">
        <Card title="Lo más visto" description="por película o serie">
          {stats.top.length === 0 ? (
            <EmptyState icon={Trophy} title="Nada todavía" />
          ) : (
            <RankTable items={stats.top} trophy />
          )}
        </Card>
        <div className="column-stack">
          <Card title="Por biblioteca">
            <RankTable items={stats.libraries} />
          </Card>
          <Card title="Por perfil">
            <RankTable items={stats.profiles} />
          </Card>
        </div>
      </div>
    </>
  )
}

function RankTable({ items, trophy = false }: { items: TitleTotal[]; trophy?: boolean }) {
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
                <Meter value={top > 0 ? item.seconds / top : 0} small ramp label={item.title} />
              </div>
            </td>
          </tr>
        ))}
      </tbody>
    </table>
  )
}

/** La etiqueta del eje para la columna `i`: no todas llevan, para que no se encimen. */
function axisLabel(stats: ViewingStats, i: number) {
  const key = stats.buckets[i].day
  const count = stats.buckets.length
  if (stats.monthly) {
    const every = Math.ceil(count / 12)
    if (i % every !== 0) return ''
    const date = parse(key)
    const month = date.toLocaleDateString('es-AR', { month: 'short' }).replace('.', '')
    return i === 0 || date.getMonth() === 0 ? `${month} ${String(date.getFullYear()).slice(2)}` : month
  }
  const date = parse(key)
  if (count <= 7) return date.toLocaleDateString('es-AR', { weekday: 'short', day: 'numeric' }).replace('.', '')
  // Por día en un mes: una etiqueta por semana, los lunes.
  return date.getDay() === 1 ? date.toLocaleDateString('es-AR', { day: 'numeric', month: 'short' }).replace('.', '') : ''
}

/** Un tope redondo en horas (o en minutos, si no llega a una hora), así la escala se lee fácil. */
function niceDuration(seconds: number) {
  const unit = seconds >= 3600 ? 3600 : 60
  return niceMax(seconds / unit) * unit
}

/** "lunes 12 de octubre" */
function dayName(day: string) {
  return parse(day).toLocaleDateString('es-AR', { weekday: 'long', day: 'numeric', month: 'long' })
}

/** "octubre de 2026" */
function monthName(month: string) {
  return parse(month).toLocaleDateString('es-AR', { month: 'long', year: 'numeric' })
}

/** "12 de octubre de 2026" */
function longDate(day: string) {
  return parse(day).toLocaleDateString('es-AR', { day: 'numeric', month: 'long', year: 'numeric' })
}

/** "2026-10-12" o "2026-10", a mediodía local (así ningún huso horario la corre de día). */
function parse(key: string) {
  return new Date(`${key.length === 7 ? `${key}-01` : key}T12:00:00`)
}
