import { useMemo, useState } from 'react'
import { useParams } from 'react-router'
import { useApi } from '../../shared/useApi'
import type { Library } from '../../shared/types/Library'
import type { Content } from '../../shared/types/Content'
import { ContentCard, SearchBox, searchTitles, titleCount } from '../catalog'
import { FocusGroup, FocusableButton } from '../focus'
import { Message } from './HomePage'

const ORDERS = {
  title: { label: 'A–Z', compare: (a: Content, b: Content) => a.title.localeCompare(b.title, 'es') },
  recent: { label: 'Recientes', compare: (a: Content, b: Content) => b.created_at - a.created_at },
  // Sin año, al final.
  year: { label: 'Año', compare: (a: Content, b: Content) => (b.year ?? 0) - (a.year ?? 0) },
}
type Order = keyof typeof ORDERS

/** Una biblioteca entera: todas sus carátulas en grilla, con búsqueda por título y orden. */
export function LibraryPage() {
  const id = Number(useParams().id)
  const { data: libraries } = useApi<Library[]>('/api/libraries')
  const { data: contents, error } = useApi<Content[]>(`/api/contents?library_id=${id}`)
  const [query, setQuery] = useState('')
  const [order, setOrder] = useState<Order>('title')
  const library = libraries?.find((l) => l.id === id)

  const shown = useMemo(
    () => searchTitles(contents ?? [], query).sort(ORDERS[order].compare),
    [contents, query, order],
  )

  if (error) return <Message title="Sin conexión con el servidor" detail={error} />
  if (!contents || !libraries) return <Message title="Cargando…" />

  return (
    <FocusGroup className="screen" autoFocus preferredChildFocusKey="GRID">
      <header className="library-header">
        <div className="library-title">
          <h1>{library?.name ?? 'Biblioteca'}</h1>
          <span className="meta">{titleCount(contents.length)}</span>
        </div>
        <FocusGroup className="library-tools">
          <div className="chips" role="group" aria-label="Orden">
            {(Object.keys(ORDERS) as Order[]).map((key) => (
              <FocusableButton
                key={key}
                className={`chip ${order === key ? 'selected' : ''}`}
                pressed={order === key}
                onPress={() => setOrder(key)}
              >
                {ORDERS[key].label}
              </FocusableButton>
            ))}
          </div>
          <SearchBox value={query} onChange={setQuery} placeholder={`Buscar en ${library?.name ?? 'la biblioteca'}`} />
        </FocusGroup>
      </header>

      {contents.length === 0 ? (
        <p className="meta">Esta biblioteca todavía está vacía.</p>
      ) : shown.length === 0 ? (
        <p className="meta">Nada coincide con "{query}".</p>
      ) : (
        <FocusGroup className="library-grid" focusKey="GRID">
          {shown.map((content) => (
            <ContentCard key={content.id} content={content} sub={content.year} />
          ))}
        </FocusGroup>
      )}
    </FocusGroup>
  )
}
