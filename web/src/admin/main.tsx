import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { HashRouter } from 'react-router'
import { App } from './App'
import '@fontsource-variable/dm-sans'
import '../shared/base.css'
import './admin.css'

// HashRouter (/admin/#/libraries): funciona con cualquier servidor de archivos estáticos.
createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <HashRouter>
      <App />
    </HashRouter>
  </StrictMode>,
)
