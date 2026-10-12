import { NavLink, Navigate, Route, Routes } from 'react-router'
import { Activity, Database, ExternalLink, LibraryBig, Settings, Upload } from 'lucide-react'
import { useEffect } from 'react'
import { useApi } from '../shared/useApi'
import type { Health } from '../shared/types/Health'
import { LibrariesPage } from './pages/LibrariesPage'
import { LibraryPage } from './pages/LibraryPage'
import { ImportPage } from './pages/ImportPage'
import { DatabasePage } from './pages/DatabasePage'
import { StatusPage } from './pages/StatusPage'
import { SettingsPage } from './pages/SettingsPage'

export function App() {
  return (
    <div className="layout">
      <Sidebar />
      <main className="content">
        <div className="page">
          <Routes>
            <Route path="/" element={<Navigate to="/status" replace />} />
            <Route path="/status" element={<StatusPage />} />
            <Route path="/libraries" element={<LibrariesPage />} />
            <Route path="/libraries/:id" element={<LibraryPage />} />
            <Route path="/import" element={<ImportPage />} />
            <Route path="/settings" element={<SettingsPage />} />
            <Route path="/database" element={<DatabasePage />} />
            <Route path="/database/:table" element={<DatabasePage />} />
          </Routes>
        </div>
      </main>
    </div>
  )
}

function Sidebar() {
  // Cada 15 segundos se comprueba que el servidor responda.
  const { data: health, error, reload } = useApi<Health>('/api/health')
  useEffect(() => {
    const timer = setInterval(reload, 15_000)
    return () => clearInterval(timer)
  }, [reload])

  return (
    <nav className="sidebar">
      <div className="brand">
        <div className="brand-mark">K</div>
        <span className="brand-name">Kinora</span>
        <span className="brand-tag">Admin</span>
      </div>

      <div className="nav">
        <NavLink to="/status">
          <Activity /> Estado
        </NavLink>
        <NavLink to="/libraries">
          <LibraryBig /> Bibliotecas
        </NavLink>
        <NavLink to="/import">
          <Upload /> Importar
        </NavLink>
        <NavLink to="/settings">
          <Settings /> Configuración
        </NavLink>
        <div className="nav-section">Depuración</div>
        <NavLink to="/database">
          <Database /> Base de datos
        </NavLink>
      </div>

      <div className="sidebar-footer">
        <a href="/" target="_blank" rel="noreferrer">
          Abrir el Player <ExternalLink />
        </a>
        <span className="row">
          <span className={`status-dot ${error ? 'off' : ''}`} />
          {error ? 'Sin conexión' : health ? `Servidor ${health.version}` : '…'}
        </span>
      </div>
    </nav>
  )
}
