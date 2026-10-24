import { defineConfig, type Plugin } from 'vite'
import react from '@vitejs/plugin-react'

// Dos aplicaciones en un mismo proyecto:
//   /        → Player (index.html)
//   /admin/  → Admin  (admin/index.html)
export default defineConfig({
  plugins: [react(), adminTrailingSlash(), noImportMeta()],
  build: {
    // Las TV: la Samsung de 2019 (Tizen 5.0) trae Chromium 63, y la VIDAA, un Chromium entre 74
    // y 79. Se compila para la más vieja: no entiende `?.`, `??` ni `catch` sin variable. Lo que
    // falta del lenguaje (`flatMap`, `Object.fromEntries`…) lo agrega `shared/polyfills.ts`.
    target: 'chrome63',
    cssTarget: 'chrome63',
    rolldownOptions: {
      input: { player: 'index.html', admin: 'admin/index.html' },
    },
  },
  server: {
    // En desarrollo, Vite reenvía la API al servidor de Kinora.
    proxy: { '/api': 'http://localhost:8090' },
  },
})

// Sin esto, el servidor de desarrollo responde /admin (sin barra) con la página del Player.
// Kinora ya redirige /admin → /admin/; acá se hace lo mismo.
function adminTrailingSlash(): Plugin {
  return {
    name: 'admin-trailing-slash',
    configureServer(server) {
      server.middlewares.use((req, res, next) => {
        if (req.url === '/admin' || req.url?.startsWith('/admin?')) {
          res.writeHead(307, { Location: '/admin/' + req.url.slice('/admin'.length) })
          res.end()
          return
        }
        next()
      })
    },
  }
}

/**
 * `import.meta` es de Chrome 64: el navegador de la Samsung (Chromium 63) ni siquiera puede leer
 * un archivo que lo tenga, y el Player entero queda en blanco. Lo trae el ayudante de Vite para
 * cargar módulos de a partes (`import.meta.url`), que React Router incluye aunque Kinora no lo
 * use. Se reemplaza por la dirección de la página, que acá da lo mismo (todo cuelga de `/`), y
 * si queda algún `import.meta` la compilación falla: así no se descubre en la TV.
 */
function noImportMeta(): Plugin {
  return {
    name: 'no-import-meta',
    apply: 'build',
    renderChunk(code) {
      if (!code.includes('import.meta')) return null
      return code.replaceAll('import.meta.resolve', 'undefined').replaceAll('import.meta.url', 'self.location.href')
    },
    generateBundle(_, bundle) {
      for (const chunk of Object.values(bundle)) {
        if (chunk.type === 'chunk' && chunk.code.includes('import.meta')) {
          this.error(`${chunk.fileName} still uses import.meta, which the Samsung TV (Chromium 63) cannot parse`)
        }
      }
    },
  }
}
