import { useEffect } from 'react'
import { Route, Routes, useLocation, useNavigate } from 'react-router'
import { HomePage } from './pages/HomePage'
import { DetailPage } from './pages/DetailPage'
import { PlayPage } from './pages/PlayPage'

/** Teclas de "Atrás": Escape y Backspace en la compu; 10009 es la tecla Atrás del control de Samsung. */
export function isBackKey(event: KeyboardEvent) {
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
    <Routes>
      <Route path="/" element={<HomePage />} />
      <Route path="/content/:id" element={<DetailPage />} />
      <Route path="/play/:id" element={<PlayPage />} />
    </Routes>
  )
}
