// Piezas para recorrer el catálogo: la carátula con su título y el campo de búsqueda.

import { useFocusable } from '@noriginmedia/norigin-spatial-navigation'
import { Search } from 'lucide-react'
import type { Content } from '../shared/types/Content'
import { artUrl } from '../shared/api'
import { Poster } from './Poster'
import { FocusableLink } from './focus'

/** "1 título", "12 títulos". */
export function titleCount(count: number) {
  return count === 1 ? '1 título' : `${count} títulos`
}

/** Una carátula que abre el detalle. Debajo, el título y una línea opcional (el año, por ejemplo). */
export function ContentCard({ content, sub }: { content: Content; sub?: string | number | null }) {
  return (
    <FocusableLink to={`/content/${content.id}`} className="card">
      <Poster title={content.title} src={artUrl(content.id, 'poster', content.poster_at)} />
      <span className="card-title">{content.title}</span>
      {sub && <span className="card-sub">{sub}</span>}
    </FocusableLink>
  )
}

/**
 * La búsqueda, enfocable con las flechas. Izquierda y derecha mueven el cursor dentro del texto;
 * arriba y abajo salen del campo, e izquierda también si el cursor ya está al principio (para
 * llegar al menú). En la TV, al enfocarla aparece el teclado de pantalla.
 */
export function SearchBox({
  value,
  onChange,
  placeholder = 'Buscar',
  className = '',
}: {
  value: string
  onChange: (value: string) => void
  placeholder?: string
  className?: string
}) {
  const { ref, focused, focusSelf } = useFocusable<object, HTMLInputElement>({
    focusKey: 'SEARCH',
    onArrowPress: (direction): boolean => {
      const input: HTMLInputElement | null = ref.current
      const atStart = input?.selectionStart === 0 && input.selectionEnd === 0
      return direction === 'up' || direction === 'down' || (direction === 'left' && atStart)
    },
  })
  return (
    <label className={`search ${className} ${focused ? 'is-focused' : ''}`}>
      <Search />
      <input
        ref={ref}
        type="search"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        onFocus={() => focusSelf()}
        placeholder={placeholder}
        aria-label="Buscar por título"
      />
    </label>
  )
}
