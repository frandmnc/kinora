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

/** Pasos de la rampa secuencial (azul, de claro a oscuro): `--seq-1` … `--seq-5` en admin.css. */
const RAMP_STEPS = 5

/**
 * Tendencia de los últimos minutos en barras finas: una por medición. Cuanto más alta, más
 * oscura y más opaca (en modo oscuro, más clara): el color refuerza la altura, así un pico se
 * nota de un vistazo. Es una rampa de un solo tono, no colores de estado: alto no es "mal".
 * Con `range`, la escala va del mínimo al máximo de lo medido (para valores que casi no se
 * mueven, como la memoria); si no, de 0 a `max` (o al máximo medido, si es mayor).
 */
export function TrendBars({
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
  const low = range && values.length > 0 ? Math.min(...values) : 0
  const high = Math.max(max ?? 0, ...values, 0)
  // En modo rango se deja un margen abajo, para que el mínimo no quede en cero.
  const span = range ? Math.max(high - low, high * 0.02, 1e-9) : Math.max(high, 1e-9)
  const base = range ? low - span * 0.25 : 0
  const scale = range ? span * 1.25 : span
  const share = (value: number) => Math.min(1, Math.max(0, (value - base) / scale))

  return (
    <div className="trend-bars" role="img" aria-label={label} title={label}>
      {values.map((value, i) => {
        const t = share(value)
        const step = Math.min(RAMP_STEPS, 1 + Math.floor(t * RAMP_STEPS))
        return (
          <span
            key={i}
            style={{
              height: `${Math.max(t * 100, 6)}%`,
              background: `var(--seq-${step})`,
              opacity: 0.4 + 0.6 * t,
            }}
          />
        )
      })}
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
 * Barras de pocos valores (por ejemplo, los últimos 7 días), con la misma rampa que
 * `TrendBars`: más alta, más oscura. Cada barra tiene su texto al pasar el mouse.
 */
export function MiniBars({ values, labels, label }: { values: number[]; labels: string[]; label: string }) {
  const top = Math.max(...values, 1e-9)
  return (
    <div className="trend-bars days" role="img" aria-label={label}>
      {values.map((value, i) => {
        const t = value / top
        const step = Math.min(RAMP_STEPS, 1 + Math.floor(t * RAMP_STEPS))
        return (
          <span
            key={labels[i]}
            title={labels[i]}
            style={{
              height: `${Math.max(t * 100, value > 0 ? 10 : 5)}%`,
              background: `var(--seq-${step})`,
              opacity: 0.4 + 0.6 * t,
            }}
          />
        )
      })}
    </div>
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
