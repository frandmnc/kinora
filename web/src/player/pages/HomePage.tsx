import { useApi } from '../../shared/useApi'
import { formatDuration } from '../../shared/format'
import type { Library } from '../../shared/types/Library'
import type { Content } from '../../shared/types/Content'
import type { ContinueItem } from '../../shared/types/ContinueItem'
import { Poster } from '../Poster'
import { FocusGroup, FocusableLink } from '../focus'

/** Inicio: "Seguir viendo" y una fila de carátulas por biblioteca. */
export function HomePage() {
  const { data: libraries, error: librariesError } = useApi<Library[]>('/api/libraries')
  const { data: contents, error: contentsError } = useApi<Content[]>('/api/contents')
  const { data: continueItems } = useApi<ContinueItem[]>('/api/continue-watching')
  const error = librariesError ?? contentsError

  if (error) return <Message title="Sin conexión con el servidor" detail={error} />
  if (!libraries || !contents || !continueItems) return <Message title="Cargando…" />

  const shelves = libraries
    .map((library) => ({ library, items: contents.filter((c) => c.library_id === library.id) }))
    .filter((shelf) => shelf.items.length > 0)

  if (shelves.length === 0) {
    return <Message title="Todavía no hay contenido" detail="Importalo desde el Admin, en /admin." />
  }

  return (
    <FocusGroup className="screen" autoFocus>
      <header className="brand-bar">Kinora</header>

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

      {shelves.map(({ library, items }) => (
        <section key={library.id} className="shelf">
          <h2>{library.name}</h2>
          <FocusGroup className="shelf-row">
            {items.map((content) => (
              <FocusableLink key={content.id} to={`/content/${content.id}`} className="card">
                <Poster title={content.title} />
                <span className="card-title">{content.title}</span>
                {content.year && <span className="card-sub">{content.year}</span>}
              </FocusableLink>
            ))}
          </FocusGroup>
        </section>
      ))}
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
        <Poster title={item.top_title} />
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

export function Message({ title, detail }: { title: string; detail?: string }) {
  return (
    <div className="message">
      <h1>{title}</h1>
      {detail && <p>{detail}</p>}
    </div>
  )
}
