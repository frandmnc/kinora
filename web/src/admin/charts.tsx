// Gráficos de líneas para el historial del servidor y de columnas para lo visto (SVG a mano:
// sin librerías).
//
// - Una sola escala por gráfico; para medidas distintas, varios gráficos uno debajo del otro.
// - Los datos van en tinta; una serie de contexto, en gris.
// - Los huecos (el servidor apagado) cortan la línea en lugar de unir puntos lejanos.
// - Al pasar el mouse, una línea vertical marca el momento en todos los gráficos a la vez y
//   la cabecera de cada uno muestra los valores de ese momento.

import { useEffect, useRef, useState, type MouseEvent } from 'react'
import { niceMax, ramp } from './ui'

export type Series<P> = {
  label: string
  value: (point: P) => number
  /** `ink`: el dato principal. `context`: una referencia en gris (por ejemplo, todo el equipo). */
  tone: 'ink' | 'context'
}

const HEIGHT = 120
const PADDING = { top: 8, right: 8, bottom: 22, left: 44 }

export function HistoryChart<P>({
  title,
  slots,
  series,
  max,
  format,
  axisLabel,
  hover,
  onHover,
}: {
  title: string
  /** Un lugar por período, en orden; `null` si en ese período no hubo mediciones. */
  slots: { t: number; point: P | null }[]
  series: Series<P>[]
  /** Tope del eje; sin tope, se toma del máximo de los datos. */
  max?: number
  format: (value: number) => string
  /** Etiqueta del eje de tiempo para el lugar `i` (vacía si no lleva). */
  axisLabel: (t: number, i: number) => string
  /** El lugar bajo el mouse (compartido entre gráficos), o `null`. */
  hover: number | null
  onHover: (index: number | null) => void
}) {
  const ref = useRef<HTMLDivElement>(null)
  const width = useWidth(ref)
  const n = slots.length
  const innerWidth = Math.max(1, width - PADDING.left - PADDING.right)
  const innerHeight = HEIGHT - PADDING.top - PADDING.bottom

  const values = slots.flatMap(({ point }) => (point ? series.map((s) => s.value(point)) : []))
  const top = niceMax(max ?? Math.max(0, ...values))
  const x = (i: number) => PADDING.left + (n <= 1 ? 0 : (i / (n - 1)) * innerWidth)
  const y = (value: number) => PADDING.top + innerHeight - (Math.min(value, top) / top) * innerHeight

  // Una línea por serie, cortada en los huecos.
  function path(s: Series<P>) {
    let d = ''
    let drawing = false
    slots.forEach(({ point }, i) => {
      if (!point) {
        drawing = false
        return
      }
      d += `${drawing ? 'L' : 'M'}${x(i).toFixed(1)},${y(s.value(point)).toFixed(1)}`
      drawing = true
    })
    return d
  }

  function onMove(event: MouseEvent<SVGRectElement>) {
    const box = event.currentTarget.getBoundingClientRect()
    const share = (event.clientX - box.left) / box.width
    onHover(Math.max(0, Math.min(n - 1, Math.round(share * (n - 1)))))
  }

  // La cabecera muestra el momento bajo el mouse o, si no, el último con datos.
  const shownIndex = hover ?? findLast(slots, (s) => s.point !== null)
  const shown = shownIndex !== null ? slots[shownIndex]?.point : null
  const ticks = [0, top / 2, top]

  return (
    <div className="history-chart" ref={ref}>
      <div className="history-chart-header">
        <strong>{title}</strong>
        <div className="history-legend">
          {series.map((s) => (
            <span key={s.label} className="history-legend-item">
              <span className={`history-swatch ${s.tone}`} />
              {s.label}
              <b>{shown ? format(s.value(shown)) : '—'}</b>
            </span>
          ))}
        </div>
      </div>
      {width > 0 && (
        <svg width={width} height={HEIGHT} role="img" aria-label={`${title}: gráfico de líneas`}>
          {ticks.map((tick) => (
            <g key={tick}>
              <line className="history-grid" x1={PADDING.left} x2={width - PADDING.right} y1={y(tick)} y2={y(tick)} />
              <text className="history-axis" x={PADDING.left - 6} y={y(tick)} textAnchor="end" dominantBaseline="middle">
                {format(tick)}
              </text>
            </g>
          ))}
          {slots.map(({ t }, i) => {
            const label = axisLabel(t, i)
            return label ? (
              <text key={t} className="history-axis" x={x(i)} y={HEIGHT - 6} textAnchor="middle">
                {label}
              </text>
            ) : null
          })}
          {/* El contexto primero, para que el dato principal quede arriba. */}
          {[...series]
            .sort((a, b) => (a.tone === b.tone ? 0 : a.tone === 'context' ? -1 : 1))
            .map((s) => (
              <path key={s.label} className={`history-line ${s.tone}`} d={path(s)} />
            ))}
          {hover !== null && (
            <g>
              <line className="history-crosshair" x1={x(hover)} x2={x(hover)} y1={PADDING.top} y2={PADDING.top + innerHeight} />
              {shown &&
                series.map((s) => (
                  <circle key={s.label} className={`history-dot ${s.tone}`} cx={x(hover)} cy={y(s.value(shown))} r={4} />
                ))}
            </g>
          )}
          {/* Zona de mouse más grande que las líneas: todo el ancho del gráfico. */}
          <rect
            className="history-hit"
            x={PADDING.left}
            y={0}
            width={innerWidth}
            height={HEIGHT}
            onMouseMove={onMove}
            onMouseLeave={() => onHover(null)}
          />
        </svg>
      )}
    </div>
  )
}

/**
 * Columnas de un total por período (por ejemplo, tiempo visto por día). Al pasar el mouse, la
 * cabecera muestra el período y su valor; si no, `summary`.
 */
export function ColumnChart({
  title,
  summary,
  columns,
  format,
  scale = niceMax,
}: {
  title: string
  summary: string
  /** En orden. `label` va en el eje (vacía si no lleva) y `name` en la cabecera al pasar el mouse. */
  columns: { name: string; label: string; value: number }[]
  format: (value: number) => string
  /** El tope del eje para el máximo de los datos. */
  scale?: (max: number) => number
}) {
  const ref = useRef<HTMLDivElement>(null)
  const width = useWidth(ref)
  const [hover, setHover] = useState<number | null>(null)
  const n = columns.length
  const innerWidth = Math.max(1, width - PADDING.left - PADDING.right)
  const innerHeight = COLUMN_HEIGHT - PADDING.top - PADDING.bottom
  const highest = Math.max(0, ...columns.map((c) => c.value))
  const top = scale(highest)
  const band = innerWidth / Math.max(1, n)
  const barWidth = Math.min(40, Math.max(2, band * 0.7))
  const center = (i: number) => PADDING.left + band * (i + 0.5)
  const y = (value: number) => PADDING.top + innerHeight - (Math.min(value, top) / top) * innerHeight
  const hovered = hover !== null ? columns[hover] : null

  function onMove(event: MouseEvent<SVGRectElement>) {
    const box = event.currentTarget.getBoundingClientRect()
    const i = Math.floor(((event.clientX - box.left) / box.width) * n)
    setHover(Math.max(0, Math.min(n - 1, i)))
  }

  return (
    <div className="history-chart" ref={ref}>
      <div className="history-chart-header">
        <strong>{title}</strong>
        <span className="history-legend">
          {hovered ? (
            <span className="history-legend-item">
              {hovered.name}
              <b>{format(hovered.value)}</b>
            </span>
          ) : (
            summary
          )}
        </span>
      </div>
      {width > 0 && (
        <svg width={width} height={COLUMN_HEIGHT} role="img" aria-label={`${title}: gráfico de columnas`}>
          {[0, top / 2, top].map((tick) => (
            <g key={tick}>
              <line className="history-grid" x1={PADDING.left} x2={width - PADDING.right} y1={y(tick)} y2={y(tick)} />
              <text className="history-axis" x={PADDING.left - 6} y={y(tick)} textAnchor="end" dominantBaseline="middle">
                {format(tick)}
              </text>
            </g>
          ))}
          {columns.map((column, i) => (
            <g key={column.name}>
              {column.value > 0 && (
                <rect
                  className="column-bar"
                  style={columnStyle(column.value / highest, hover === null ? null : hover === i)}
                  x={center(i) - barWidth / 2}
                  y={y(column.value)}
                  width={barWidth}
                  height={Math.max(1, PADDING.top + innerHeight - y(column.value))}
                  rx={Math.min(3, barWidth / 4)}
                />
              )}
              {column.label && (
                <text className="history-axis" x={center(i)} y={COLUMN_HEIGHT - 6} textAnchor="middle">
                  {column.label}
                </text>
              )}
            </g>
          ))}
          <rect
            className="history-hit"
            x={PADDING.left}
            y={0}
            width={innerWidth}
            height={COLUMN_HEIGHT}
            onMouseMove={onMove}
            onMouseLeave={() => setHover(null)}
          />
        </svg>
      )}
    </div>
  )
}

const COLUMN_HEIGHT = 180

/**
 * Las columnas usan la rampa de la brasa del panel de Estado: cuanto más alta, más intensa. Con el
 * mouse encima de una (`active`), las demás se apagan.
 */
function columnStyle(share: number, active: boolean | null) {
  const { color, opacity } = ramp(share)
  return { fill: color, opacity: active === true ? 1 : active === false ? opacity * 0.4 : opacity }
}

function findLast<T>(items: T[], test: (item: T) => boolean): number | null {
  for (let i = items.length - 1; i >= 0; i--) if (test(items[i])) return i
  return null
}

/** El ancho del elemento, actualizado si cambia (al achicar la ventana). */
function useWidth(ref: React.RefObject<HTMLDivElement | null>) {
  const [width, setWidth] = useState(0)
  useEffect(() => {
    const element = ref.current
    if (!element) return
    const observer = new ResizeObserver(([entry]) => setWidth(Math.floor(entry.contentRect.width)))
    observer.observe(element)
    return () => observer.disconnect()
  }, [ref])
  return width
}
