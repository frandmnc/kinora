import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { HashRouter } from 'react-router'
import { App } from './App'
import '../shared/base.css'
import './player.css'

// HashRouter (/#/content/3): funciona con cualquier servidor de archivos y en la app de la TV.
createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <HashRouter>
      <App />
    </HashRouter>
  </StrictMode>,
)
