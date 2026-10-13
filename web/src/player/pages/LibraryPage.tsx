import { useMemo, useState } from 'react'
import { useParams } from 'react-router'
import { useFocusable } from '@noriginmedia/norigin-spatial-navigation'
import { ArrowLeft, Search } from 'lucide-react'
import { useApi } from '../../shared/useApi'
import type { Library } from '../../shared/types/Library'
import type { Content } from '../../shared/types/Content'
import { Poster } from '../Poster'
import { FocusGroup, FocusableLink } from '../focus'
import { Message } from './HomePage'

/** Para buscar sin que importen los acentos ni las mayúsculas: "Pokémon" encuentra "pokemon". */
function normalize(text: string) {
  return text.normalize('NFD').replace(/\p{M}/gu, '').toLowerCase()
}

/** Una biblioteca entera: todas sus carátulas en grilla, con búsqueda por título. */
export function LibraryPage() {
  const id = Number(useParams().id)
  const { data: libraries } = useApi<Library[]>('/api/libraries')
  const { data: contents, error } = useApi<Content[]>(`/api/contents?library_id=${id}`)
  const [query, setQuery] = useState('')
  const library = libraries?.find((l) => l.id === id)

  const shown = useMemo(() => {
    const words = normalize(query).split(/\s+/).filter(Boolean)
    return (contents ?? []).filter((c) => words.every((word) => normalize(c.title).includes(word)))
  }, [contents, query])

  if (error) return <Message title="Sin conexión con el servidor" detail={error} />
  if (!contents || !libraries) return <Message title="Cargando…" />

  return (
    <FocusGroup className="screen" autoFocus preferredChildFocusKey="GRID">
      <FocusableLink to="/" className="back">
        <ArrowLeft /> Inicio
      </FocusableLink>
      <header className="library-header">
        <h1>{library?.name ?? 'Biblioteca'}</h1>
        <span className="meta">{contents.length} títulos</span>
        <SearchBox value={query} onChange={setQuery} />
      </header>

      {shown.length === 0 ? (
        <p className="meta">Nada coincide con "{query}".</p>
      ) : (
        <FocusGroup className="library-grid" focusKey="GRID">
          {shown.map((content) => (
            <FocusableLink key={content.id} to={`/content/${content.id}`} className="card">
              <Poster title={content.title} />
              <span className="card-title">{content.title}</span>
              {content.year && <span className="card-sub">{content.year}</span>}
            </FocusableLink>
          ))}
        </FocusGroup>
      )}
    </FocusGroup>
  )
}

/**
 * La búsqueda, enfocable con las flechas. Izquierda y derecha mueven el cursor dentro del texto;
 * arriba y abajo salen del campo. En la TV, al enfocarla aparece el teclado de pantalla.
 */
function SearchBox({ value, onChange }: { value: string; onChange: (value: string) => void }) {
  const { ref, focused, focusSelf } = useFocusable<object, HTMLInputElement>({
    onArrowPress: (direction) => direction === 'up' || direction === 'down',
  })
  return (
    <label className={`search ${focused ? 'is-focused' : ''}`}>
      <Search />
      <input
        ref={ref}
        type="search"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        onFocus={() => focusSelf()}
        placeholder="Buscar"
        aria-label="Buscar por título"
      />
    </label>
  )
}
