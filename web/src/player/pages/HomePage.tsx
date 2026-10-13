import { LayoutGrid } from 'lucide-react'
import { useApi } from '../../shared/useApi'
import { formatDuration } from '../../shared/format'
import type { Library } from '../../shared/types/Library'
import type { Content } from '../../shared/types/Content'
import type { ContinueItem } from '../../shared/types/ContinueItem'
import type { RecentItem } from '../../shared/types/RecentItem'
import { Poster } from '../Poster'
import { FocusGroup, FocusableButton, FocusableLink } from '../focus'
import { Avatar, useProfile } from '../profiles'

/** Cuántas carátulas muestra cada biblioteca en el inicio; el resto, en "Ver todo". */
const SHELF_ITEMS = 15

/** Inicio: "Seguir viendo", "Agregados recientemente" y una fila de carátulas por biblioteca. */
export function HomePage() {
  const { profile, switchProfile } = useProfile()
  const { data: libraries, error: librariesError } = useApi<Library[]>('/api/libraries')
  const { data: contents, error: contentsError } = useApi<Content[]>('/api/contents')
  const { data: continueItems } = useApi<ContinueItem[]>('/api/continue-watching')
  const { data: recent } = useApi<RecentItem[]>('/api/recent')
  const error = librariesError ?? contentsError

  if (error) return <Message title="Sin conexión con el servidor" detail={error} />
  // Se espera a tener todo: el foco inicial va al primer elemento, y si una fila llega tarde
  // aparece arriba del foco y lo deja en el medio de la pantalla.
  if (!libraries || !contents || !continueItems || !recent) return <Message title="Cargando…" />

  const shelves = libraries
    .map((library) => ({ library, items: contents.filter((c) => c.library_id === library.id) }))
    .filter((shelf) => shelf.items.length > 0)

  if (shelves.length === 0) {
    return <Message title="Todavía no hay contenido" detail="Importalo desde el Admin, en /admin." />
  }

  return (
    <FocusGroup className="screen" autoFocus>
      <header className="home-header">
        <span className="brand-bar">Kinora</span>
        <FocusableButton className="profile-chip" onPress={switchProfile} title="Cambiar de perfil">
          <Avatar id={profile.id} name={profile.name} size="small" />
          {profile.name}
        </FocusableButton>
      </header>

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
              <FocusableLink key={content.id} to={`/content/${content.id}`} className="card">
                <Poster title={content.title} />
                <span className="card-title">{content.title}</span>
                <span className="card-sub">{[note || content.year, ago(added_at)].filter(Boolean).join(' · ')}</span>
              </FocusableLink>
            ))}
          </FocusGroup>
        </section>
      )}

      {shelves.map(({ library, items }) => (
        <section key={library.id} className="shelf">
          <h2>{library.name}</h2>
          <FocusGroup className="shelf-row">
            {items.slice(0, SHELF_ITEMS).map((content) => (
              <FocusableLink key={content.id} to={`/content/${content.id}`} className="card">
                <Poster title={content.title} />
                <span className="card-title">{content.title}</span>
                {content.year && <span className="card-sub">{content.year}</span>}
              </FocusableLink>
            ))}
            <FocusableLink to={`/library/${library.id}`} className="card see-all">
              <div className="poster see-all-poster">
                <LayoutGrid />
                <strong>Ver todo</strong>
                <span>{items.length} títulos</span>
              </div>
            </FocusableLink>
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
