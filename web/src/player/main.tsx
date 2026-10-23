import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { HashRouter } from 'react-router'
import { init } from '@noriginmedia/norigin-spatial-navigation'
import { App } from './App'
import '@fontsource/barlow/400.css'
import '@fontsource/barlow/500.css'
import '@fontsource/barlow/600.css'
import '@fontsource/barlow-condensed/300.css'
import '@fontsource/barlow-condensed/500.css'
import '@fontsource-variable/inter'
import '../shared/base.css'
import './player.css'

// La letra de los subtítulos. El navegador baja las fuentes recién cuando algo las usa, y un
// subtítulo que aparece antes se dibujaría con otra: se pide de entrada.
document.fonts.load('600 1em "Inter Variable"').catch(() => {})

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
