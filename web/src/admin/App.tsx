import { NavLink, Navigate, Route, Routes } from 'react-router'
import { LibrariesPage } from './pages/LibrariesPage'
import { ImportPage } from './pages/ImportPage'
import { DatabasePage } from './pages/DatabasePage'
import { StatusPage } from './pages/StatusPage'
import { SettingsPage } from './pages/SettingsPage'

export function App() {
  return (
    <div className="layout">
      <nav className="sidebar">
        <div className="brand">
          Kinora <span>Admin</span>
        </div>
        <NavLink to="/status">Estado</NavLink>
        <NavLink to="/libraries">Bibliotecas</NavLink>
        <NavLink to="/import">Importar</NavLink>
        <NavLink to="/settings">Configuración</NavLink>
        <div className="nav-section">Depuración</div>
        <NavLink to="/database">Base de datos</NavLink>
      </nav>
      <main className="content">
        <Routes>
          <Route path="/" element={<Navigate to="/status" replace />} />
          <Route path="/status" element={<StatusPage />} />
          <Route path="/libraries" element={<LibrariesPage />} />
          <Route path="/import" element={<ImportPage />} />
          <Route path="/settings" element={<SettingsPage />} />
          <Route path="/database" element={<DatabasePage />} />
          <Route path="/database/:table" element={<DatabasePage />} />
        </Routes>
      </main>
    </div>
  )
}
