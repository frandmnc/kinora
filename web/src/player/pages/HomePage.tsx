import { Link } from 'react-router'
import { useApi } from '../../shared/useApi'
import type { Library } from '../../shared/types/Library'
import type { Content } from '../../shared/types/Content'
import { Poster } from '../Poster'

/** Inicio: una fila de carátulas por biblioteca. */
export function HomePage() {
  const { data: libraries, error: librariesError } = useApi<Library[]>('/api/libraries')
  const { data: contents, error: contentsError } = useApi<Content[]>('/api/contents')
  const error = librariesError ?? contentsError

  if (error) return <Message title="Sin conexión con el servidor" detail={error} />
  if (!libraries || !contents) return <Message title="Cargando…" />

  const sections = libraries
    .map((library) => ({ library, items: contents.filter((c) => c.library_id === library.id) }))
    .filter((section) => section.items.length > 0)

  if (sections.length === 0) {
    return <Message title="Todavía no hay contenido" detail="Importalo desde el Admin, en /admin." />
  }

  return (
    <div className="screen">
      <header className="brand-bar">Kinora</header>
      {sections.map(({ library, items }) => (
        <section key={library.id} className="shelf">
          <h2>{library.name}</h2>
          <div className="shelf-row">
            {items.map((content) => (
              <Link key={content.id} to={`/content/${content.id}`} className="card">
                <Poster content={content} />
                <span className="card-title">{content.title}</span>
                {content.year && <span className="card-year">{content.year}</span>}
              </Link>
            ))}
          </div>
        </section>
      ))}
    </div>
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
