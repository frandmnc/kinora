import { useState, type FormEvent } from 'react'
import { KeyRound, Laptop, X } from 'lucide-react'
import { api } from '../shared/api'
import { useApi } from '../shared/useApi'
import type { SessionInfo } from '../shared/types/SessionInfo'
import type { PasswordChange } from '../shared/types/PasswordChange'
import { Alert, Badge, Card } from './ui'

/** La cuenta: cambiar la contraseña y ver (y cerrar) las sesiones abiertas en cada equipo. */
export function AccountCard() {
  const { data: sessions, error, reload } = useApi<SessionInfo[]>('/api/admin/account/sessions')
  const [changing, setChanging] = useState(false)
  const [notice, setNotice] = useState<string>()
  const [actionError, setActionError] = useState<string>()

  async function close(session: SessionInfo) {
    if (!confirm(`¿Cerrar la sesión de ${session.device ?? 'ese equipo'}? Va a tener que volver a entrar.`)) return
    setActionError(undefined)
    try {
      await api(`/api/admin/account/sessions/${session.id}`, { method: 'DELETE' })
      reload()
    } catch (err) {
      setActionError((err as Error).message)
    }
  }

  return (
    <Card
      title="Cuenta"
      description="las sesiones duran 180 días desde el último uso"
      actions={
        !changing && (
          <button className="btn btn-sm" onClick={() => setChanging(true)}>
            <KeyRound /> Cambiar la contraseña
          </button>
        )
      }
    >
      {changing && (
        <PasswordForm
          onDone={() => {
            setChanging(false)
            setNotice('Contraseña cambiada. Se cerraron las sesiones de los demás equipos.')
            reload()
          }}
          onCancel={() => setChanging(false)}
        />
      )}
      {notice && (
        <div className="card-body">
          <Alert tone="info">{notice}</Alert>
        </div>
      )}
      {(error || actionError) && (
        <div className="card-body">
          <Alert>{error ?? actionError}</Alert>
        </div>
      )}
      {sessions?.map((session) => (
        <div key={session.id} className="list-row">
          <Laptop className="icon" />
          <span className="grow">
            <strong>{session.device ?? 'Equipo desconocido'}</strong>{' '}
            {session.current && <Badge tone="good">Este equipo</Badge>}
            <div className="small muted">
              {session.client} · entró {when(session.created_at)} · usado {when(session.last_seen_at)}
            </div>
          </span>
          {!session.current && (
            <button className="btn btn-ghost btn-sm btn-danger" onClick={() => close(session)}>
              <X /> Cerrar
            </button>
          )}
        </div>
      ))}
    </Card>
  )
}

function PasswordForm({ onDone, onCancel }: { onDone: () => void; onCancel: () => void }) {
  const [current, setCurrent] = useState('')
  const [next, setNext] = useState('')
  const [repeat, setRepeat] = useState('')
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string>()

  async function submit(event: FormEvent) {
    event.preventDefault()
    if (next !== repeat) {
      setError('Las contraseñas nuevas no coinciden.')
      return
    }
    setBusy(true)
    setError(undefined)
    try {
      const body: PasswordChange = { current, new: next }
      await api('/api/admin/account/password', { method: 'PUT', body })
      onDone()
    } catch (err) {
      setError((err as Error).message)
      setBusy(false)
    }
  }

  return (
    <form className="card-body password-form" onSubmit={submit}>
      <div className="fields">
        <label className="field">
          Contraseña actual
          <input
            className="input"
            type="password"
            value={current}
            onChange={(e) => setCurrent(e.target.value)}
            autoComplete="current-password"
            required
            autoFocus
          />
        </label>
        <label className="field">
          Nueva
          <input
            className="input"
            type="password"
            value={next}
            onChange={(e) => setNext(e.target.value)}
            autoComplete="new-password"
            minLength={8}
            required
          />
        </label>
        <label className="field">
          Repetir la nueva
          <input
            className="input"
            type="password"
            value={repeat}
            onChange={(e) => setRepeat(e.target.value)}
            autoComplete="new-password"
            required
          />
        </label>
      </div>
      <p className="small muted">
        Al menos 8 caracteres. Se cierran las sesiones de los demás equipos (por si la anterior se filtró).
      </p>
      {error && <Alert>{error}</Alert>}
      <div className="row">
        <button className="btn btn-primary" type="submit" disabled={busy}>
          {busy ? 'Un momento…' : 'Cambiar'}
        </button>
        <button className="btn btn-ghost" type="button" onClick={onCancel} disabled={busy}>
          Cancelar
        </button>
      </div>
    </form>
  )
}

/** "hoy 11:46", "ayer 22:10" o "12 oct". */
function when(seconds: number) {
  const date = new Date(seconds * 1000)
  const midnight = (d: Date) => new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime()
  const days = Math.round((midnight(new Date()) - midnight(date)) / 86_400_000)
  const time = date.toLocaleTimeString('es-AR', { hour: '2-digit', minute: '2-digit', hour12: false })
  if (days === 0) return `hoy ${time}`
  if (days === 1) return `ayer ${time}`
  return date.toLocaleDateString('es-AR', { day: 'numeric', month: 'short' })
}
