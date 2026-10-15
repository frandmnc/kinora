import { NavLink, Navigate, Route, Routes } from 'react-router'
import {
  Activity,
  ChartColumn,
  Database,
  ExternalLink,
  LibraryBig,
  ListOrdered,
  LogOut,
  Settings,
  Trash2,
  Upload,
} from 'lucide-react'
import { useEffect } from 'react'
import { useApi } from '../shared/useApi'
import { AuthGate } from '../shared/auth'
import type { Health } from '../shared/types/Health'
import { LibrariesPage } from './pages/LibrariesPage'
import { LibraryPage } from './pages/LibraryPage'
import { ContentPage } from './pages/ContentPage'
import { ImportPage } from './pages/ImportPage'
import { JobsPage } from './pages/JobsPage'
import { TrashPage } from './pages/TrashPage'
import { DatabasePage } from './pages/DatabasePage'
import { StatusPage } from './pages/StatusPage'
import { ViewingPage } from './pages/ViewingPage'
import { SettingsPage } from './pages/SettingsPage'

export function App() {
  return (
    <AuthGate brand={<Brand />}>
      {(state, signOut) => <Layout username={state.username} onSignOut={signOut} />}
    </AuthGate>
  )
}

function Brand() {
  return (
    <div className="brand">
      <div className="brand-mark">K</div>
      <span className="brand-name">Kinora</span>
      <span className="brand-tag">Admin</span>
    </div>
  )
}

function Layout({ username, onSignOut }: { username: string | null; onSignOut: () => Promise<void> }) {
  return (
    <div className="layout">
      <Sidebar username={username} onSignOut={onSignOut} />
      <main className="content">
        <div className="page">
          <Routes>
            <Route path="/" element={<Navigate to="/status" replace />} />
            <Route path="/status" element={<StatusPage />} />
            <Route path="/viewing" element={<ViewingPage />} />
            <Route path="/libraries" element={<LibrariesPage />} />
            <Route path="/libraries/:id" element={<LibraryPage />} />
            <Route path="/libraries/:libraryId/contents/:id" element={<ContentPage />} />
            <Route path="/import" element={<ImportPage />} />
            <Route path="/jobs" element={<JobsPage />} />
            <Route path="/trash" element={<TrashPage />} />
            <Route path="/settings" element={<SettingsPage />} />
            <Route path="/database" element={<DatabasePage />} />
            <Route path="/database/:table" element={<DatabasePage />} />
          </Routes>
        </div>
      </main>
    </div>
  )
}

function Sidebar({ username, onSignOut }: { username: string | null; onSignOut: () => Promise<void> }) {
  // Cada 15 segundos se comprueba que el servidor responda.
  const { data: health, error, reload } = useApi<Health>('/api/health')
  useEffect(() => {
    const timer = setInterval(reload, 15_000)
    return () => clearInterval(timer)
  }, [reload])

  return (
    <nav className="sidebar">
      <Brand />

      <div className="nav">
        <NavLink to="/status">
          <Activity /> Estado
        </NavLink>
        <NavLink to="/viewing">
          <ChartColumn /> Visualizaciones
        </NavLink>
        <NavLink to="/libraries">
          <LibraryBig /> Bibliotecas
        </NavLink>
        <NavLink to="/import">
          <Upload /> Importar
        </NavLink>
        <NavLink to="/jobs">
          <ListOrdered /> Cola
        </NavLink>
        <NavLink to="/trash">
          <Trash2 /> Papelera
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
        <button className="sidebar-signout" onClick={onSignOut} title="Cerrar la sesión en este equipo">
          <LogOut /> Salir{username && <span className="muted"> ({username})</span>}
        </button>
      </div>
    </nav>
  )
}
