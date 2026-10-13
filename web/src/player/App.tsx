import { useEffect } from 'react'
import { Route, Routes, useLocation, useNavigate } from 'react-router'
import { pause, resume } from '@noriginmedia/norigin-spatial-navigation'
import { AuthGate } from '../shared/auth'
import { HomePage } from './pages/HomePage'
import { DetailPage } from './pages/DetailPage'
import { LibraryPage } from './pages/LibraryPage'
import { PlayPage } from './pages/PlayPage'

/** Teclas de "Atrás": Escape y Backspace en la compu; 10009 es la tecla Atrás del control de Samsung. */
export function isBackKey(event: KeyboardEvent) {
  // Escribiendo (en la búsqueda), Backspace borra: no es "Atrás".
  const typing = event.target instanceof HTMLInputElement || event.target instanceof HTMLTextAreaElement
  if (event.key === 'Backspace' && typing) return false
  return event.key === 'Escape' || event.key === 'Backspace' || event.key === 'GoBack' || event.keyCode === 10009
}

export function App() {
  const navigate = useNavigate()
  const location = useLocation()

  useEffect(() => {
    function onKey(event: KeyboardEvent) {
      // En pantalla completa, Escape primero sale de pantalla completa (lo hace el navegador).
      if (!isBackKey(event) || document.fullscreenElement || location.pathname === '/') return
      event.preventDefault()
      // Si se entró directo a esta pantalla no hay historial: se vuelve al inicio.
      const hasHistory = (window.history.state?.idx ?? 0) > 0
      if (hasHistory) navigate(-1)
      else navigate('/', { replace: true })
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [location.pathname, navigate])

  return (
    <AuthGate brand={<PlayerBrand />}>
      {() => <PlayerRoutes />}
    </AuthGate>
  )
}

/**
 * El logo de las pantallas de entrada. Mientras se ven, la navegación con flechas se apaga:
 * se escribe en campos comunes, y Enter tiene que enviar el formulario.
 */
function PlayerBrand() {
  useEffect(() => {
    pause()
    return () => resume()
  }, [])
  return <span className="brand-bar">Kinora</span>
}

function PlayerRoutes() {
  return (
    <Routes>
      <Route path="/" element={<HomePage />} />
      <Route path="/library/:id" element={<LibraryPage />} />
      <Route path="/content/:id" element={<DetailPage />} />
      <Route path="/play/:id" element={<PlayPage />} />
    </Routes>
  )
}
