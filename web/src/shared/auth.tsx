// Entrada a Kinora: crear la cuenta (la primera vez) o iniciar sesión. Lo usan el Admin y el
// Player; cada uno le da su estilo a las clases `auth-*`.

import { useCallback, useEffect, useState, type FormEvent, type ReactNode } from 'react'
import { api, SIGNED_OUT } from './api'
import type { AuthState } from './types/AuthState'

/**
 * Muestra `children` solo con sesión. Si no hay cuenta, la pantalla para crearla; si no hay
 * sesión, la de entrar. Si en algún momento el servidor contesta 401, vuelve a entrar.
 */
export function AuthGate({
  children,
  brand,
  setupElsewhere,
}: {
  children: (state: AuthState, signOut: () => Promise<void>) => ReactNode
  /** Lo que va arriba del formulario (el logo de cada app). */
  brand: ReactNode
  /** Qué mostrar si la cuenta no existe y no se puede crear desde acá. */
  setupElsewhere?: ReactNode
}) {
  const [state, setState] = useState<AuthState>()
  const [error, setError] = useState<string>()

  const check = useCallback(() => {
    api<AuthState>('/api/auth/state')
      .then((result) => {
        setState(result)
        setError(undefined)
      })
      .catch((err: Error) => setError(err.message))
  }, [])

  useEffect(() => {
    check()
    window.addEventListener(SIGNED_OUT, check)
    return () => window.removeEventListener(SIGNED_OUT, check)
  }, [check])

  const signOut = useCallback(async () => {
    await api('/api/auth/logout', { method: 'POST' })
    check()
  }, [check])

  if (error) {
    return (
      <AuthScreen brand={brand}>
        <p className="auth-error">Sin conexión con el servidor: {error}</p>
        <button className="auth-button" onClick={check}>
          Reintentar
        </button>
      </AuthScreen>
    )
  }
  if (!state) return null
  if (state.logged_in) return <>{children(state, signOut)}</>
  if (state.setup_needed && !state.can_setup_here) {
    return (
      <AuthScreen brand={brand} title="Kinora todavía no tiene cuenta">
        {setupElsewhere ?? (
          <p className="auth-text">
            Creala desde la compu donde corre Kinora (o desde la red de casa): por seguridad, no se puede crear desde
            afuera.
          </p>
        )}
      </AuthScreen>
    )
  }
  return <CredentialsForm brand={brand} setup={state.setup_needed} onDone={check} />
}

function CredentialsForm({ brand, setup, onDone }: { brand: ReactNode; setup: boolean; onDone: () => void }) {
  const [username, setUsername] = useState('')
  const [password, setPassword] = useState('')
  const [repeat, setRepeat] = useState('')
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string>()

  async function submit(event: FormEvent) {
    event.preventDefault()
    if (setup && password !== repeat) {
      setError('Las contraseñas no coinciden.')
      return
    }
    setBusy(true)
    setError(undefined)
    try {
      await api(setup ? '/api/auth/setup' : '/api/auth/login', {
        method: 'POST',
        body: { username, password },
      })
      onDone()
    } catch (err) {
      setError((err as Error).message)
      setBusy(false)
    }
  }

  return (
    <AuthScreen
      brand={brand}
      title={setup ? 'Creá la cuenta de Kinora' : 'Iniciá sesión'}
      subtitle={
        setup
          ? 'Es la única cuenta: después vas a poder crear perfiles para cada persona de la casa.'
          : 'La sesión queda abierta en este equipo.'
      }
    >
      <form
        className="auth-form"
        onSubmit={submit}
        // En el Player, la navegación con flechas cancela el Enter (aunque esté en pausa) y el
        // formulario no se enviaría solo: se envía a mano.
        onKeyDown={(e) => {
          if (e.key === 'Enter' && e.target instanceof HTMLInputElement) {
            e.preventDefault()
            e.currentTarget.requestSubmit()
          }
        }}
      >
        <label className="auth-field">
          Usuario
          <input
            value={username}
            onChange={(e) => setUsername(e.target.value)}
            autoComplete="username"
            autoCapitalize="none"
            autoFocus
            required
          />
        </label>
        <label className="auth-field">
          Contraseña
          <input
            type="password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            autoComplete={setup ? 'new-password' : 'current-password'}
            minLength={setup ? 8 : undefined}
            required
          />
        </label>
        {setup && (
          <label className="auth-field">
            Repetir la contraseña
            <input
              type="password"
              value={repeat}
              onChange={(e) => setRepeat(e.target.value)}
              autoComplete="new-password"
              required
            />
            <span className="auth-hint">Al menos 8 caracteres.</span>
          </label>
        )}
        {error && <p className="auth-error">{error}</p>}
        <button className="auth-button" type="submit" disabled={busy}>
          {busy ? 'Un momento…' : setup ? 'Crear la cuenta' : 'Entrar'}
        </button>
      </form>
    </AuthScreen>
  )
}

function AuthScreen({
  brand,
  title,
  subtitle,
  children,
}: {
  brand: ReactNode
  title?: string
  subtitle?: string
  children: ReactNode
}) {
  return (
    <main className="auth-screen">
      <div className="auth-card">
        <div className="auth-brand">{brand}</div>
        {title && <h1>{title}</h1>}
        {subtitle && <p className="auth-text">{subtitle}</p>}
        {children}
      </div>
    </main>
  )
}
