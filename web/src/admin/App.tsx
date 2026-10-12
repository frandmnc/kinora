import { NavLink, Navigate, Route, Routes } from 'react-router'
import { LibrariesPage } from './pages/LibrariesPage'
import { ImportPage } from './pages/ImportPage'
import { DatabasePage } from './pages/DatabasePage'

export function App() {
  return (
    <div className="layout">
      <nav className="sidebar">
        <div className="brand">
          Kinora <span>Admin</span>
        </div>
        <NavLink to="/libraries">Bibliotecas</NavLink>
        <NavLink to="/import">Importar</NavLink>
        <div className="nav-section">Depuración</div>
        <NavLink to="/database">Base de datos</NavLink>
      </nav>
      <main className="content">
        <Routes>
          <Route path="/" element={<Navigate to="/libraries" replace />} />
          <Route path="/libraries" element={<LibrariesPage />} />
          <Route path="/import" element={<ImportPage />} />
          <Route path="/database" element={<DatabasePage />} />
          <Route path="/database/:table" element={<DatabasePage />} />
        </Routes>
      </main>
    </div>
  )
}
