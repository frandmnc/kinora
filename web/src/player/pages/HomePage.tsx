import { useApi } from '../../shared/useApi'
import { formatDuration } from '../../shared/format'
import type { ContinueItem } from '../../shared/types/ContinueItem'
import type { RecentItem } from '../../shared/types/RecentItem'
import { artUrl } from '../../shared/api'
import { Poster } from '../Poster'
import { ContentCard } from '../catalog'
import { FocusGroup, FocusableLink } from '../focus'

/** Inicio: "Seguir viendo" y "Agregados recientemente". Las bibliotecas, desde el menú. */
export function HomePage() {
  const { data: continueItems, error: continueError } = useApi<ContinueItem[]>('/api/continue-watching')
  const { data: recent, error: recentError } = useApi<RecentItem[]>('/api/recent')
  const error = continueError ?? recentError

  if (error) return <Message title="Sin conexión con el servidor" detail={error} />
  // Se espera a tener todo: el foco inicial va al primer elemento, y si una fila llega tarde
  // aparece arriba del foco y lo deja en el medio de la pantalla.
  if (!continueItems || !recent) return <Message title="Cargando…" />

  // "Agregados recientemente" sale de los archivos importados: si está vacío, no hay nada.
  if (continueItems.length === 0 && recent.length === 0) {
    return <Message title="Todavía no hay contenido" detail="Importalo desde el Admin, en /admin." />
  }

  return (
    <FocusGroup className="screen" autoFocus>
      {continueItems.length > 0 && (
        <section className="shelf">
          <h2>Seguir viendo</h2>
          <FocusGroup className="shelf-row">
            {continueItems.map((item) => (
              <ContinueCard key={item.content_id} item={item} />
            ))}
          </FocusGroup>
        </section>
      )}

      {recent.length > 0 && (
        <section className="shelf">
          <h2>Agregados recientemente</h2>
          <FocusGroup className="shelf-row">
            {recent.map(({ content, note, added_at }) => (
              <ContentCard
                key={content.id}
                content={content}
                sub={[note || content.year, ago(added_at)].filter(Boolean).join(' · ')}
              />
            ))}
          </FocusGroup>
        </section>
      )}

    </FocusGroup>
  )
}

function ContinueCard({ item }: { item: ContinueItem }) {
  const share = item.duration ? item.position / item.duration : 0
  const remaining = item.duration ? item.duration - item.position : null
  // "Sousou no Frieren · T1 E2" → "T1 E2" debajo del título de la serie.
  const episode = item.title.startsWith(item.top_title) ? item.title.slice(item.top_title.length + 3) : ''

  return (
    <FocusableLink to={`/play/${item.content_id}`} state={{ title: item.title }} className="card">
      <div className="poster-wrap">
        <Poster
          title={item.top_title}
          src={artUrl(item.top_id, 'poster', item.top_poster_at)}
        />
        {share > 0 && (
          <div className="poster-progress">
            <div style={{ width: `${share * 100}%` }} />
          </div>
        )}
      </div>
      <span className="card-title">{item.top_title}</span>
      <span className="card-sub">
        {[episode, item.next_up ? 'Siguiente episodio' : remaining ? `quedan ${formatDuration(remaining)}` : null]
          .filter(Boolean)
          .join(' · ')}
      </span>
    </FocusableLink>
  )
}

/** "hoy", "ayer", "hace 3 días", "hace 2 semanas". */
function ago(seconds: number) {
  // Días de calendario: algo de anoche a las 23 h es "ayer" aunque no hayan pasado 24 horas.
  const midnight = (date: Date) => new Date(date.getFullYear(), date.getMonth(), date.getDate()).getTime()
  const days = Math.round((midnight(new Date()) - midnight(new Date(seconds * 1000))) / 86_400_000)
  if (days < 1) return 'hoy'
  if (days === 1) return 'ayer'
  if (days < 14) return `hace ${days} días`
  return `hace ${Math.floor(days / 7)} semanas`
}

export function Message({ title, detail }: { title: string; detail?: string }) {
  return (
    <div className="message">
      <h1>{title}</h1>
      {detail && <p>{detail}</p>}
    </div>
  )
}
