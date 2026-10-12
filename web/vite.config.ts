import { defineConfig, type Plugin } from 'vite'
import react from '@vitejs/plugin-react'

// Dos aplicaciones en un mismo proyecto:
//   /        → Player (index.html)
//   /admin/  → Admin  (admin/index.html)
export default defineConfig({
  plugins: [react(), adminTrailingSlash()],
  build: {
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
