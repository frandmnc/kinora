import { useApi } from '../shared/useApi'
import type { Health } from '../shared/types/Health'

/** Pantalla provisoria del Player. La grilla y la reproducción llegan con la importación. */
export function App() {
  const { data: health, error } = useApi<Health>('/api/health')

  return (
    <div className="player">
      <h1>Kinora</h1>
      <p>Todavía no hay contenido para ver.</p>
      <p className="status">
        {error ? `Sin conexión con el servidor: ${error}` : health ? `Servidor ${health.version} conectado` : '…'}
      </p>
    </div>
  )
}
