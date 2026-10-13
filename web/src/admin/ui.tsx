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

/**
 * Línea de tendencia mínima: trazo de 2 px en gris de segundo plano, un relleno muy suave
 * y el punto actual en el color de acento (con anillo del color de la superficie).
 *
 * Por defecto la escala arranca en cero. Con `range`, va del mínimo al máximo de los valores:
 * sirve para medidas que casi no varían (como la memoria), donde desde cero la línea sería plana.
 */
export function Sparkline({
  values,
  max,
  range,
  label,
}: {
  values: number[]
  max?: number
  range?: boolean
  label: string
}) {
  const width = 200
  const height = 34
  const pad = 5
  if (values.length < 2) return <div className="sparkline" aria-hidden="true" />

  const low = range ? Math.min(...values) : 0
  const high = Math.max(max ?? 0, ...values)
  // En modo rango se deja un margen arriba y abajo; si no hay variación, la línea queda al medio.
  const span = range ? Math.max(high - low, high * 0.02, 1e-9) : Math.max(high, 1e-9)
  const base = range ? low - span * 0.25 : 0
  const scale = range ? span * 1.5 : span
  const x = (i: number) => pad + (i / (values.length - 1)) * (width - pad * 2)
  const y = (v: number) => height - pad - ((v - base) / scale) * (height - pad * 2)
  const line = values.map((v, i) => `${i === 0 ? 'M' : 'L'}${x(i).toFixed(1)},${y(v).toFixed(1)}`).join(' ')
  const area = `${line} L${x(values.length - 1).toFixed(1)},${height - pad} L${x(0).toFixed(1)},${height - pad} Z`
  const last = values.length - 1

  // La línea se estira con la tarjeta (`preserveAspectRatio="none"`), pero un círculo estirado
  // se vuelve óvalo: el punto final va aparte, en HTML, ubicado en porcentajes.
  return (
    <div className="sparkline" role="img" aria-label={label}>
      <svg viewBox={`0 0 ${width} ${height}`} preserveAspectRatio="none" aria-hidden="true">
        <path d={area} fill="var(--deemph)" opacity="0.12" />
        <path
          d={line}
          fill="none"
          stroke="var(--deemph)"
          strokeWidth="2"
          strokeLinejoin="round"
          strokeLinecap="round"
          vectorEffect="non-scaling-stroke"
        />
      </svg>
      <span
        className="sparkline-dot"
        style={{ left: `${(x(last) / width) * 100}%`, top: `${(y(values[last]) / height) * 100}%` }}
      />
    </div>
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
 * Barras de pocos valores (por ejemplo, los últimos 7 días). Barras finas con la punta
 * redondeada sobre una línea base; la última (el período actual) va resaltada.
 * Cada barra tiene su texto al pasar el mouse.
 */
export function MiniBars({ values, labels, label }: { values: number[]; labels: string[]; label: string }) {
  const width = 200
  const height = 34
  const top = Math.max(...values, 1e-9)
  const slot = width / values.length
  const barWidth = Math.min(16, slot * 0.55)
  return (
    <svg className="sparkline" viewBox={`0 0 ${width} ${height}`} role="img" aria-label={label}>
      {values.map((value, i) => {
        const h = Math.max((value / top) * (height - 4), value > 0 ? 3 : 1.5)
        const x = i * slot + (slot - barWidth) / 2
        const current = i === values.length - 1
        return (
          <rect
            key={labels[i]}
            x={x}
            y={height - h}
            width={barWidth}
            height={h}
            rx={Math.min(4, barWidth / 2)}
            fill={current ? 'var(--accent)' : 'var(--deemph)'}
          >
            <title>{labels[i]}</title>
          </rect>
        )
      })}
    </svg>
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
