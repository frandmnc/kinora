// Piezas de interfaz del Admin. Los estilos están en admin.css.

import { useEffect, useRef, type ReactNode } from 'react'
import { CircleAlert, CircleCheck, Info, TriangleAlert, type LucideIcon } from 'lucide-react'

export function PageHeader({
  title,
  description,
  actions,
}: {
  title: string
  description?: ReactNode
  actions?: ReactNode
}) {
  return (
    <header className="page-header">
      <div>
        <h1>{title}</h1>
        {description && <p>{description}</p>}
      </div>
      {actions && <div className="page-actions">{actions}</div>}
    </header>
  )
}

export function Card({
  title,
  description,
  actions,
  children,
  footer,
}: {
  title?: ReactNode
  description?: ReactNode
  actions?: ReactNode
  children?: ReactNode
  footer?: ReactNode
}) {
  return (
    <section className="card">
      {(title || actions) && (
        <div className="card-header">
          {title && <h2>{title}</h2>}
          {description && <span className="description">{description}</span>}
          {actions && <div className="actions">{actions}</div>}
        </div>
      )}
      {children}
      {footer && <div className="card-footer">{footer}</div>}
    </section>
  )
}

type Tone = 'critical' | 'warning' | 'info'
const alertIcons: Record<Tone, LucideIcon> = { critical: CircleAlert, warning: TriangleAlert, info: Info }

/** Aviso con ícono: el color nunca carga el significado solo. */
export function Alert({ tone = 'critical', children }: { tone?: Tone; children: ReactNode }) {
  const Icon = alertIcons[tone]
  return (
    <div className={`alert alert-${tone}`} role={tone === 'critical' ? 'alert' : 'status'}>
      <Icon />
      <div>{children}</div>
    </div>
  )
}

export function Badge({
  tone,
  icon: Icon,
  children,
}: {
  tone?: 'accent' | 'good' | 'warning' | 'critical'
  icon?: LucideIcon
  children: ReactNode
}) {
  return (
    <span className={`badge ${tone ? `badge-${tone}` : ''}`}>
      {Icon && <Icon />}
      {children}
    </span>
  )
}

export function OkBadge({ children }: { children: ReactNode }) {
  return (
    <Badge tone="good" icon={CircleCheck}>
      {children}
    </Badge>
  )
}

export function EmptyState({ icon: Icon, title, children }: { icon: LucideIcon; title: string; children?: ReactNode }) {
  return (
    <div className="empty">
      <Icon />
      <strong>{title}</strong>
      {children && <span>{children}</span>}
    </div>
  )
}

/**
 * Medidor de 0 a 1. El relleno lleva el estado (acento → atención → crítico) y la pista
 * es un tono claro del mismo color, así el estado se lee en toda la barra.
 */
export function Meter({
  value,
  warnAt,
  criticalAt,
  small,
  label,
}: {
  value: number
  warnAt?: number
  criticalAt?: number
  small?: boolean
  label?: string
}) {
  const clamped = Math.max(0, Math.min(1, value))
  const tone =
    criticalAt !== undefined && clamped >= criticalAt
      ? 'critical'
      : warnAt !== undefined && clamped >= warnAt
        ? 'warning'
        : ''
  return (
    <div
      className={`meter ${tone} ${small ? 'meter-sm' : ''}`}
      role="meter"
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={Math.round(clamped * 100)}
      aria-label={label}
    >
      <span style={{ width: `${clamped * 100}%` }} />
    </div>
  )
}

/** Pasos de la rampa secuencial (coral, de claro a intenso): `--seq-1` … `--seq-5` en admin.css. */
const RAMP_STEPS = 5

/** Un tope "redondo" para la escala: 1, 2, 2,5, 5 o 10 por la potencia de 10 que corresponda. */
export function niceMax(value: number) {
  if (value <= 0) return 1
  const power = 10 ** Math.floor(Math.log10(value))
  return [1, 2, 2.5, 5, 10].map((m) => m * power).find((m) => m >= value) ?? 10 * power
}

/** Una barra de la rampa: la altura (0..1) decide el tono y la opacidad. */
function rampBar(share: number, minHeight: number, title: string, key: number | string) {
  const step = Math.min(RAMP_STEPS, 1 + Math.floor(share * RAMP_STEPS))
  return (
    <span
      key={key}
      title={title}
      style={{
        height: `${Math.max(share * 100, minHeight)}%`,
        background: `var(--seq-${step})`,
        opacity: 0.45 + 0.55 * share,
      }}
    />
  )
}

/**
 * Barras con su escala a la derecha: arriba el tope (con una línea de referencia punteada) y
 * abajo el piso, para saber si una barra alta es mucho o poco.
 */
function ScaledBars({
  bars,
  top,
  bottom,
  label,
  className = '',
}: {
  bars: ReactNode
  top: string
  bottom: string
  label: string
  className?: string
}) {
  return (
    <div className="trend" role="img" aria-label={`${label} (escala de ${bottom} a ${top})`}>
      <div className={`trend-bars ${className}`}>{bars}</div>
      <div className="trend-scale" aria-hidden="true">
        <span>{top}</span>
        <span>{bottom}</span>
      </div>
    </div>
  )
}

/**
 * Tendencia de los últimos minutos en barras finas: una por medición. Cuanto más alta, más
 * intensa y más opaca: el color refuerza la altura, así un pico se nota de un vistazo.
 * La escala va de 0 a `max` (o a un tope redondo por encima de lo medido, si es mayor). Con
 * `range`, va del mínimo al máximo medido, para valores que casi no se mueven (la memoria).
 * Al pasar el mouse por una barra se ve su valor y hace cuánto se midió.
 */
export function TrendBars({
  values,
  max,
  range,
  label,
  format,
  every = 2,
}: {
  values: number[]
  max?: number
  range?: boolean
  label: string
  format: (value: number) => string
  /** Segundos entre mediciones. */
  every?: number
}) {
  const highest = Math.max(0, ...values)
  const lowest = values.length > 0 ? Math.min(...values) : 0
  let floor = 0
  let ceiling = niceMax(Math.max(max ?? 0, highest))
  if (range) {
    // Un margen arriba y abajo de lo medido; si no varía, la barra queda a media altura.
    const spread = Math.max(highest - lowest, highest * 0.02, 1e-9)
    floor = Math.max(0, lowest - spread)
    ceiling = highest + spread
  }
  const share = (value: number) => Math.min(1, Math.max(0, (value - floor) / (ceiling - floor)))
  const ago = (i: number) => {
    const seconds = (values.length - 1 - i) * every
    return seconds === 0 ? 'ahora' : seconds < 60 ? `hace ${seconds} s` : `hace ${Math.round(seconds / 60)} min`
  }

  return (
    <ScaledBars
      label={label}
      top={format(ceiling)}
      bottom={format(floor)}
      bars={values.map((value, i) => rampBar(share(value), 4, `${format(value)} · ${ago(i)}`, i))}
    />
  )
}

/** Tile de un número: etiqueta, valor, detalle y opcionalmente una tendencia o un medidor. */
export function StatTile({
  icon: Icon,
  label,
  value,
  sub,
  children,
}: {
  icon: LucideIcon
  label: string
  value: string
  sub?: ReactNode
  children?: ReactNode
}) {
  return (
    <div className="card tile">
      <div className="tile-label">
        <Icon />
        {label}
      </div>
      <div className="tile-value">{value}</div>
      {sub && <div className="tile-sub">{sub}</div>}
      {children}
    </div>
  )
}

/**
 * Barras de pocos valores (por ejemplo, los últimos 7 días), con la misma rampa y escala que
 * `TrendBars`. Cada barra tiene su texto al pasar el mouse.
 */
export function MiniBars({
  values,
  labels,
  label,
  format,
}: {
  values: number[]
  labels: string[]
  label: string
  format: (value: number) => string
}) {
  const ceiling = Math.max(...values, 0)
  const top = ceiling > 0 ? ceiling : 1
  return (
    <ScaledBars
      className="days"
      label={label}
      top={ceiling > 0 ? format(ceiling) : '—'}
      bottom={format(0)}
      bars={values.map((value, i) => rampBar(value / top, value > 0 ? 8 : 4, labels[i], labels[i]))}
    />
  )
}

/**
 * Ventana de confirmación (un `<dialog>` modal: Escape la cierra y el foco queda adentro).
 * Para acciones que no se deshacen fácil, como eliminar.
 */
export function ConfirmDialog({
  title,
  children,
  confirmLabel,
  danger,
  busy,
  disabled,
  onConfirm,
  onCancel,
}: {
  title: string
  children: ReactNode
  confirmLabel: string
  danger?: boolean
  busy?: boolean
  disabled?: boolean
  onConfirm: () => void
  onCancel: () => void
}) {
  const ref = useRef<HTMLDialogElement>(null)
  useEffect(() => {
    ref.current?.showModal()
  }, [])

  return (
    <dialog
      ref={ref}
      className="dialog"
      onCancel={(e) => {
        e.preventDefault()
        if (!busy) onCancel()
      }}
      onClick={(e) => {
        // Un clic afuera de la ventana (en el fondo) también cancela.
        if (e.target === e.currentTarget && !busy) onCancel()
      }}
    >
      <div className="dialog-body">
        <h2>{title}</h2>
        {children}
      </div>
      <div className="dialog-actions">
        <button className="btn btn-ghost" onClick={onCancel} disabled={busy}>
          Cancelar
        </button>
        <button
          className={`btn ${danger ? 'btn-danger-solid' : 'btn-primary'}`}
          onClick={onConfirm}
          disabled={busy || disabled}
          autoFocus={!danger}
        >
          {busy ? 'Un momento…' : confirmLabel}
        </button>
      </div>
    </dialog>
  )
}
