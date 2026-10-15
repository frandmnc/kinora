import { useMemo, useState } from 'react'
import { useApi } from '../../shared/useApi'
import type { Library } from '../../shared/types/Library'
import type { Content } from '../../shared/types/Content'
import { ContentCard, SearchBox, searchTitles, titleCount } from '../catalog'
import { FocusGroup } from '../focus'
import { Message } from './HomePage'

/** Buscar por título en todas las bibliotecas a la vez. El foco arranca en el campo. */
export function SearchPage() {
  const { data: libraries } = useApi<Library[]>('/api/libraries')
  const { data: contents, error } = useApi<Content[]>('/api/contents')
  const [query, setQuery] = useState('')

  const found = useMemo(() => (query.trim() ? searchTitles(contents ?? [], query) : []), [contents, query])

  if (error) return <Message title="Sin conexión con el servidor" detail={error} />
  if (!contents || !libraries) return <Message title="Cargando…" />

  const libraryName = (id: number) => libraries.find((l) => l.id === id)?.name

  return (
    <FocusGroup className="screen" autoFocus>
      <SearchBox
        className="search-large"
        value={query}
        onChange={setQuery}
        placeholder="Películas y series"
      />

      {!query.trim() ? (
        <p className="search-hint meta">Buscá por título entre {titleCount(contents.length)}.</p>
      ) : found.length === 0 ? (
        <p className="search-hint meta">Nada coincide con "{query}".</p>
      ) : (
        <>
          <p className="search-hint meta">{titleCount(found.length)}</p>
          <FocusGroup className="library-grid">
            {found.map((content) => (
              <ContentCard
                key={content.id}
                content={content}
                sub={[content.year, libraryName(content.library_id)].filter(Boolean).join(' · ')}
              />
            ))}
          </FocusGroup>
        </>
      )}
    </FocusGroup>
  )
}
