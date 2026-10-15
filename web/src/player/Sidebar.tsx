// El menú lateral: inicio, búsqueda, cada biblioteca y el perfil. En la TV es una franja de
// íconos que se abre al entrar con la flecha izquierda (o al pasar el mouse); en el celular,
// una barra abajo.

import { createContext, useContext, useEffect, useState, type ReactNode } from 'react'
import { Link, Outlet, useLocation, useNavigate } from 'react-router'
import { FocusContext, setFocus, useFocusable } from '@noriginmedia/norigin-spatial-navigation'
import { Clapperboard, House, LibraryBig, Search, Tv, type LucideIcon } from 'lucide-react'
import { useApi } from '../shared/useApi'
import type { Content } from '../shared/types/Content'
import type { Library } from '../shared/types/Library'
import { FocusGroup } from './focus'
import { Avatar, useProfile } from './profiles'

/** Claves de foco del menú y del contenido de la pantalla, para mover el foco de uno al otro. */
export const MENU = 'MENU'
export const CONTENT = 'CONTENT'

const HighlightContext = createContext<(libraryId: number | null) => void>(() => {})

/**
 * Marca en el menú la biblioteca de lo que se está viendo (en el detalle, la URL no la dice).
 * Al salir de la pantalla, la marca se borra.
 */
export function useHighlightLibrary(libraryId: number | undefined) {
  const highlight = useContext(HighlightContext)
  useEffect(() => {
    if (libraryId === undefined) return
    highlight(libraryId)
    return () => highlight(null)
  }, [libraryId, highlight])
}

/** Las pantallas para recorrer el catálogo: el menú a la izquierda y la pantalla a la derecha. */
export function BrowseLayout() {
  const [highlighted, setHighlighted] = useState<number | null>(null)
  return (
    <HighlightContext.Provider value={setHighlighted}>
      <Sidebar highlighted={highlighted} />
      <FocusGroup className="browse-main" focusKey={CONTENT}>
        <Outlet />
      </FocusGroup>
    </HighlightContext.Provider>
  )
}

function Sidebar({ highlighted }: { highlighted: number | null }) {
  const { profile, switchProfile } = useProfile()
  const { pathname } = useLocation()
  const { data: libraries } = useApi<Library[]>('/api/libraries')
  const { data: contents } = useApi<Content[]>('/api/contents')
  const pointer = usePointerMode()

  const libraryInUrl = pathname.match(/^\/library\/(\d+)/)?.[1]
  const active =
    pathname === '/'
      ? 'home'
      : pathname === '/search'
        ? 'search'
        : libraryInUrl
          ? `library-${libraryInUrl}`
          : highlighted !== null
            ? `library-${highlighted}`
            : null

  const { ref, focusKey, hasFocusedChild } = useFocusable<object, HTMLElement>({
    focusKey: MENU,
    trackChildren: true,
    // Al entrar al menú, el foco cae en la sección en la que se está, no en la última elegida.
    saveLastFocusedChild: false,
    preferredChildFocusKey: active ? `${MENU}-${active}` : undefined,
  })

  // Con las flechas, el menú se abre cuando el foco entra; con el mouse, al pasar por encima.
  const open = hasFocusedChild && !pointer

  return (
    <FocusContext.Provider value={focusKey}>
      <nav ref={ref} className={`sidebar ${open ? 'open' : ''} ${pointer ? 'pointer' : ''}`} aria-label="Menú">
        <span className="menu-brand" aria-label="Kinora">
          K<span className="menu-label">inora</span>
        </span>

        <div className="menu-items">
          <MenuItem id="home" to="/" icon={House} label="Inicio" active={active === 'home'} />
          <MenuItem id="search" to="/search" icon={Search} label="Buscar" active={active === 'search'} />
          {libraries && libraries.length > 0 && <hr className="menu-divider" />}
          {libraries?.map((library) => (
            <MenuItem
              key={library.id}
              id={`library-${library.id}`}
              to={`/library/${library.id}`}
              icon={libraryIcon(library, contents)}
              label={library.name}
              active={active === `library-${library.id}`}
            />
          ))}
        </div>

        <ProfileItem onPress={switchProfile}>
          <Avatar id={profile.id} name={profile.name} size="small" />
          <span className="menu-label menu-profile">
            {profile.name}
            <small>Cambiar perfil</small>
          </span>
        </ProfileItem>
      </nav>
    </FocusContext.Provider>
  )
}

function MenuItem({
  id,
  to,
  icon: Icon,
  label,
  active,
}: {
  id: string
  to: string
  icon: LucideIcon
  label: string
  active: boolean
}) {
  const navigate = useNavigate()
  const { pathname } = useLocation()

  // Si ya se está ahí, elegirla devuelve el foco a la pantalla (no hay nada que cargar).
  const open = () => (pathname === to ? setFocus(CONTENT) : navigate(to))

  const { ref, focused, focusSelf } = useFocusable<object, HTMLAnchorElement>({
    focusKey: `${MENU}-${id}`,
    onEnterPress: open,
  })
  return (
    <Link
      ref={ref}
      to={to}
      className={`menu-item ${active ? 'active' : ''} ${focused ? 'is-focused' : ''}`}
      aria-current={active ? 'page' : undefined}
      title={label}
      onClick={(event) => {
        event.preventDefault()
        open()
      }}
      onMouseEnter={() => focusSelf()}
    >
      <Icon className="menu-icon" aria-hidden="true" />
      <span className="menu-label">{label}</span>
    </Link>
  )
}

function ProfileItem({ onPress, children }: { onPress: () => void; children: ReactNode }) {
  const { ref, focused, focusSelf } = useFocusable<object, HTMLButtonElement>({
    focusKey: `${MENU}-profile`,
    onEnterPress: onPress,
  })
  return (
    <button
      ref={ref}
      type="button"
      className={`menu-item profile-item ${focused ? 'is-focused' : ''}`}
      onClick={onPress}
      onMouseEnter={() => focusSelf()}
      title="Cambiar de perfil"
    >
      {children}
    </button>
  )
}

/**
 * El ícono de una biblioteca, según lo que tiene: series o películas (lo que más haya).
 * Vacía, o mientras carga, un ícono genérico.
 */
function libraryIcon(library: Library, contents: Content[] | undefined): LucideIcon {
  const own = contents?.filter((c) => c.library_id === library.id) ?? []
  const shows = own.filter((c) => c.kind === 'show').length
  const movies = own.filter((c) => c.kind === 'movie').length
  if (shows + movies === 0) return LibraryBig
  return shows > movies ? Tv : Clapperboard
}

/**
 * Si lo último que se usó fue el mouse (o el dedo). Con el mouse, el foco sigue al puntero, así
 * que abrir el menú con el foco lo dejaría abierto al salir de él: ahí se abre solo al pasar.
 */
function usePointerMode() {
  const [pointer, setPointer] = useState(false)
  useEffect(() => {
    const onPointer = () => setPointer(true)
    const onKey = () => setPointer(false)
    window.addEventListener('pointermove', onPointer)
    window.addEventListener('pointerdown', onPointer)
    window.addEventListener('keydown', onKey)
    return () => {
      window.removeEventListener('pointermove', onPointer)
      window.removeEventListener('pointerdown', onPointer)
      window.removeEventListener('keydown', onKey)
    }
  }, [])
  return pointer
}
