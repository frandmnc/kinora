import { useState, type FormEvent } from 'react'
import { KeyRound, X } from 'lucide-react'
import { api } from '../shared/api'
import { useApi } from '../shared/useApi'
import type { OpenSubtitlesAccount } from '../shared/types/OpenSubtitlesAccount'
import type { OpenSubtitlesSetting } from '../shared/types/OpenSubtitlesSetting'
import { Alert, Badge, Card } from './ui'

/**
 * La cuenta de OpenSubtitles, para buscar y bajar subtítulos desde cada video. La clave y la
 * contraseña no vuelven del servidor.
 */
export function OpenSubtitlesCard() {
  const { data: setting, error, reload } = useApi<OpenSubtitlesSetting>('/api/admin/settings/opensubtitles')
  const [editing, setEditing] = useState(false)
  const [account, setAccount] = useState<OpenSubtitlesAccount>({ key: '', username: '', password: '' })
  const [busy, setBusy] = useState(false)
  const [actionError, setActionError] = useState<string>()

  function close() {
    setEditing(false)
    setAccount({ key: '', username: '', password: '' })
    setActionError(undefined)
  }

  async function save(event: FormEvent) {
    event.preventDefault()
    setBusy(true)
    setActionError(undefined)
    try {
      await api('/api/admin/settings/opensubtitles', { method: 'PUT', body: account })
      close()
      reload()
    } catch (err) {
      setActionError((err as Error).message)
    } finally {
      setBusy(false)
    }
  }

  async function remove() {
    if (!confirm('¿Quitar la cuenta de OpenSubtitles? Los subtítulos ya bajados quedan.')) return
    setActionError(undefined)
    try {
      await api('/api/admin/settings/opensubtitles', { method: 'DELETE' })
      reload()
    } catch (err) {
      setActionError((err as Error).message)
    }
  }

  const field = (name: keyof OpenSubtitlesAccount) => ({
    value: account[name],
    onChange: (e: { target: { value: string } }) => setAccount({ ...account, [name]: e.target.value }),
  })

  return (
    <Card
      title="OpenSubtitles"
      description="buscar y bajar subtítulos para cada video"
      actions={
        setting &&
        !editing && (
          <>
            {setting.configured && (
              <button className="btn btn-ghost btn-sm btn-danger" onClick={remove}>
                <X /> Quitar
              </button>
            )}
            <button
              className="btn btn-sm"
              onClick={() => {
                setAccount({ key: '', username: setting.username ?? '', password: '' })
                setEditing(true)
              }}
            >
              <KeyRound /> {setting.configured ? 'Cambiar la cuenta' : 'Cargar la cuenta'}
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
              <>
                <Badge tone="good">Cuenta cargada</Badge>
                <span className="small muted">{setting.username}</span>
              </>
            ) : (
              <span className="muted">Sin cuenta: los subtítulos se pueden subir a mano, pero no buscar.</span>
            )}
          </div>
        )}
        {editing && (
          <form className="tmdb-form" onSubmit={save}>
            <p className="small muted">
              Hace falta una cuenta gratis en{' '}
              <a href="https://www.opensubtitles.com" target="_blank" rel="noreferrer">
                opensubtitles.com
              </a>{' '}
              y una clave de la API, que se crea en{' '}
              <a href="https://www.opensubtitles.com/consumers" target="_blank" rel="noreferrer">
                tu perfil → API consumers
              </a>
              . La cuenta gratis permite unas 20 descargas por día; buscar no gasta descargas.
            </p>
            <label className="field">
              Clave de la API
              <input
                className="input mono"
                type="password"
                autoComplete="off"
                spellCheck={false}
                required
                autoFocus
                {...field('key')}
              />
            </label>
            <div className="tmdb-form-row">
              <label className="field grow">
                Usuario
                <input className="input" autoComplete="off" spellCheck={false} required {...field('username')} />
              </label>
              <label className="field grow">
                Contraseña
                <input className="input" type="password" autoComplete="off" required {...field('password')} />
              </label>
            </div>
            <div className="row">
              <button className="btn btn-primary" type="submit" disabled={busy}>
                {busy ? 'Probando…' : 'Guardar'}
              </button>
              <button className="btn btn-ghost" type="button" disabled={busy} onClick={close}>
                Cancelar
              </button>
            </div>
          </form>
        )}
      </div>
    </Card>
  )
}
