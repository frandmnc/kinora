import { useState, type FormEvent } from 'react'
import { KeyRound, X } from 'lucide-react'
import { api } from '../shared/api'
import { useApi } from '../shared/useApi'
import type { TmdbKey } from '../shared/types/TmdbKey'
import type { TmdbSetting } from '../shared/types/TmdbSetting'
import { Alert, Badge, Card } from './ui'

/** La clave de TMDB, para buscar títulos, sinopsis y carátulas. La clave no vuelve del servidor. */
export function TmdbCard() {
  const { data: setting, error, reload } = useApi<TmdbSetting>('/api/admin/settings/tmdb')
  const [editing, setEditing] = useState(false)
  const [key, setKey] = useState('')
  const [busy, setBusy] = useState(false)
  const [actionError, setActionError] = useState<string>()

  async function save(event: FormEvent) {
    event.preventDefault()
    setBusy(true)
    setActionError(undefined)
    try {
      const body: TmdbKey = { key }
      await api('/api/admin/settings/tmdb', { method: 'PUT', body })
      setEditing(false)
      setKey('')
      reload()
    } catch (err) {
      setActionError((err as Error).message)
    } finally {
      setBusy(false)
    }
  }

  async function remove() {
    if (!confirm('¿Quitar la clave de TMDB? Lo que ya se trajo queda guardado.')) return
    setActionError(undefined)
    try {
      await api('/api/admin/settings/tmdb', { method: 'DELETE' })
      reload()
    } catch (err) {
      setActionError((err as Error).message)
    }
  }

  return (
    <Card
      title="TMDB"
      description="títulos, sinopsis y carátulas de películas y series"
      actions={
        setting &&
        !editing && (
          <>
            {setting.configured && (
              <button className="btn btn-ghost btn-sm btn-danger" onClick={remove}>
                <X /> Quitar
              </button>
            )}
            <button className="btn btn-sm" onClick={() => setEditing(true)}>
              <KeyRound /> {setting.configured ? 'Cambiar la clave' : 'Cargar la clave'}
            </button>
          </>
        )
      }
    >
      <div className="card-body tmdb-body">
        {(error || actionError) && <Alert>{error ?? actionError}</Alert>}
        {setting && !editing && (
          <div className="row">
            {setting.configured ? (
              <Badge tone="good">Clave cargada</Badge>
            ) : (
              <span className="muted">
                Sin clave: la metadata se puede corregir a mano, pero no buscar en TMDB.
              </span>
            )}
          </div>
        )}
        {editing && (
          <form className="tmdb-form" onSubmit={save}>
            <p className="small muted">
              Se saca gratis en{' '}
              <a href="https://www.themoviedb.org/settings/api" target="_blank" rel="noreferrer">
                themoviedb.org → Configuración → API
              </a>{' '}
              (hace falta una cuenta). Sirve la "API Key" o el "API Read Access Token".
            </p>
            <label className="field">
              Clave
              <input
                className="input mono"
                type="password"
                value={key}
                onChange={(e) => setKey(e.target.value)}
                autoComplete="off"
                spellCheck={false}
                required
                autoFocus
              />
            </label>
            <div className="row">
              <button className="btn btn-primary" type="submit" disabled={busy || !key.trim()}>
                {busy ? 'Probando…' : 'Guardar'}
              </button>
              <button
                className="btn btn-ghost"
                type="button"
                disabled={busy}
                onClick={() => {
                  setEditing(false)
                  setKey('')
                  setActionError(undefined)
                }}
              >
                Cancelar
              </button>
            </div>
          </form>
        )}
      </div>
    </Card>
  )
}
