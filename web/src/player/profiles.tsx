// "¿Quién está mirando?": al abrir el Player se elige el perfil, y con él se ven su progreso y
// su "Seguir viendo". Desde acá también se agregan, renombran y eliminan perfiles.

import { createContext, useCallback, useContext, useEffect, useState, type ReactNode } from 'react'
import { LogOut, Pencil, Plus } from 'lucide-react'
import { api, setProfile } from '../shared/api'
import { useApi } from '../shared/useApi'
import type { Profile } from '../shared/types/Profile'
import { FocusableButton, FocusableInput, FocusGroup } from './focus'

type ProfileState = {
  profile: Profile
  /** Vuelve a "¿Quién está mirando?". */
  switchProfile: () => void
}

const ProfileContext = createContext<ProfileState | null>(null)

/** El perfil elegido (dentro de `ProfileGate`). */
export function useProfile() {
  const state = useContext(ProfileContext)
  if (!state) throw new Error('useProfile fuera de ProfileGate')
  return state
}

/**
 * Muestra `children` con un perfil elegido. Se pregunta cada vez que se abre el Player: el
 * perfil vive solo en memoria, así nadie sigue la serie en el perfil de otro sin darse cuenta.
 */
export function ProfileGate({ children, onSignOut }: { children: ReactNode; onSignOut: () => Promise<void> }) {
  const [profile, setChosen] = useState<Profile | null>(null)

  const choose = useCallback((chosen: Profile | null) => {
    setProfile(chosen?.id ?? null)
    setChosen(chosen)
  }, [])

  if (!profile) return <ProfilePicker onChoose={choose} onSignOut={onSignOut} />
  return (
    <ProfileContext.Provider value={{ profile, switchProfile: () => choose(null) }}>
      {children}
    </ProfileContext.Provider>
  )
}

/** Tonos de los avatares: bien distintos entre sí, para reconocer cada perfil de lejos. */
const AVATAR_HUES = [350, 28, 145, 190, 215, 265, 310, 48]

/**
 * Un cuadrado con la inicial. El color sale del número de perfil (no del nombre), así los
 * primeros ocho nunca se repiten y renombrar no lo cambia.
 */
export function Avatar({ id, name, size = 'large' }: { id: number; name: string; size?: 'large' | 'small' }) {
  const hue = AVATAR_HUES[(id - 1) % AVATAR_HUES.length]
  return (
    <span
      className={`avatar avatar-${size}`}
      style={{ background: `hsl(${hue}, 32%, 34%)` }}
      aria-hidden="true"
    >
      {name.charAt(0).toUpperCase()}
    </span>
  )
}

function ProfilePicker({
  onChoose,
  onSignOut,
}: {
  onChoose: (profile: Profile) => void
  onSignOut: () => Promise<void>
}) {
  const { data: profiles, error, reload } = useApi<Profile[]>('/api/profiles')
  const [managing, setManaging] = useState(false)

  if (error) {
    return (
      <div className="message">
        <h1>Sin conexión con el servidor</h1>
        <p>{error}</p>
      </div>
    )
  }
  if (!profiles) return null

  if (managing) return <ProfileManager profiles={profiles} onChange={reload} onDone={() => setManaging(false)} />

  return (
    <FocusGroup className="profiles-screen" autoFocus>
      <h1>¿Quién está mirando?</h1>
      <FocusGroup className="profile-grid">
        {profiles.map((profile) => (
          <FocusableButton key={profile.id} className="profile-tile" onPress={() => onChoose(profile)}>
            <Avatar id={profile.id} name={profile.name} />
            <span className="profile-name">{profile.name}</span>
          </FocusableButton>
        ))}
      </FocusGroup>
      <FocusGroup className="profile-actions">
        <FocusableButton className="ghost-button" onPress={() => setManaging(true)}>
          <Pencil /> Administrar perfiles
        </FocusableButton>
        <FocusableButton className="ghost-button" onPress={onSignOut} title="Cerrar la sesión en este equipo">
          <LogOut /> Salir
        </FocusableButton>
      </FocusGroup>
    </FocusGroup>
  )
}

/** Agregar, renombrar y eliminar perfiles. Eliminar pide confirmar (otro Enter). */
function ProfileManager({
  profiles,
  onChange,
  onDone,
}: {
  profiles: Profile[]
  onChange: () => void
  onDone: () => void
}) {
  const [names, setNames] = useState<Record<number, string>>({})
  const [newName, setNewName] = useState('')
  const [confirming, setConfirming] = useState<number>()
  const [error, setError] = useState<string>()

  // Cada vez que llega la lista, los campos arrancan con los nombres guardados.
  useEffect(() => {
    setNames(Object.fromEntries(profiles.map((p) => [p.id, p.name])))
  }, [profiles])

  async function run(action: () => Promise<unknown>) {
    setError(undefined)
    try {
      await action()
      onChange()
    } catch (err) {
      setError((err as Error).message)
    }
  }

  const add = () =>
    newName.trim() &&
    run(async () => {
      await api('/api/profiles', { method: 'POST', body: { name: newName } })
      setNewName('')
    })

  return (
    <FocusGroup className="profiles-screen manage" autoFocus>
      <h1>Administrar perfiles</h1>
      {profiles.map((profile) => {
        const name = names[profile.id] ?? profile.name
        const changed = name.trim() !== profile.name
        const rename = () =>
          changed && run(() => api(`/api/profiles/${profile.id}`, { method: 'PATCH', body: { name } }))
        return (
          <FocusGroup key={profile.id} className="profile-row">
            <Avatar id={profile.id} name={name || profile.name} size="small" />
            <FocusableInput
              className="profile-input"
              value={name}
              onChange={(value) => setNames({ ...names, [profile.id]: value })}
              onEnter={rename}
              label={`Nombre de ${profile.name}`}
              maxLength={20}
            />
            {changed && (
              <FocusableButton className="ghost-button" onPress={rename}>
                Guardar
              </FocusableButton>
            )}
            {profiles.length > 1 &&
              (confirming === profile.id ? (
                <FocusableButton
                  className="ghost-button danger"
                  onPress={() =>
                    run(async () => {
                      await api(`/api/profiles/${profile.id}`, { method: 'DELETE' })
                      setConfirming(undefined)
                    })
                  }
                >
                  ¿Seguro? Se borra su progreso
                </FocusableButton>
              ) : (
                <FocusableButton className="ghost-button" onPress={() => setConfirming(profile.id)}>
                  Eliminar
                </FocusableButton>
              ))}
          </FocusGroup>
        )
      })}
      <FocusGroup className="profile-row">
        <span className="avatar avatar-small avatar-new" aria-hidden="true">
          <Plus />
        </span>
        <FocusableInput
          className="profile-input"
          value={newName}
          onChange={setNewName}
          onEnter={add}
          placeholder="Nombre del perfil nuevo"
          label="Nombre del perfil nuevo"
          maxLength={20}
        />
        <FocusableButton className="ghost-button" onPress={add}>
          Agregar
        </FocusableButton>
      </FocusGroup>
      {error && <p className="auth-error">{error}</p>}
      <FocusableButton className="play-button" onPress={onDone}>
        Listo
      </FocusableButton>
    </FocusGroup>
  )
}
