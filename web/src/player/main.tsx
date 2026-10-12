import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { HashRouter } from 'react-router'
import { init } from '@noriginmedia/norigin-spatial-navigation'
import { App } from './App'
import '@fontsource-variable/dm-sans'
import '../shared/base.css'
import './player.css'

// Navegación espacial: las flechas mueven el foco al elemento más cercano en esa dirección,
// como en una TV. `shouldFocusDOMNode` hace que el foco sea real (Enter activa el link, y el
// navegador desplaza la pantalla hasta el elemento).
init({ shouldFocusDOMNode: true })

// HashRouter (/#/content/3): funciona con cualquier servidor de archivos y en la app de la TV.
createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <HashRouter>
      <App />
    </HashRouter>
  </StrictMode>,
)
