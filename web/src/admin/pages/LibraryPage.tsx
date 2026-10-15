import { useMemo, useState } from 'react'
import { Link, useParams } from 'react-router'
import { ArrowLeft, Clapperboard, ListOrdered, Search, Trash, TriangleAlert, Tv } from 'lucide-react'
import { artUrl } from '../../shared/api'
import { useApi } from '../../shared/useApi'
import { formatSize } from '../../shared/format'
import { searchTitles } from '../../shared/search'
import { languageName } from '../../shared/languages'
import type { ContentSummary } from '../../shared/types/ContentSummary'
import type { Library } from '../../shared/types/Library'
import { Alert, EmptyState, PageHeader } from '../ui'
import { DeleteContentDialog } from '../DeleteContentDialog'

const ORDERS = {
  title: { label: 'Título', compare: (a: ContentSummary, b: ContentSummary) => a.content.title.localeCompare(b.content.title, 'es') },
  recent: { label: 'Agregados recientemente', compare: (a: ContentSummary, b: ContentSummary) => b.content.created_at - a.content.created_at },
  // Sin año, al final.
  year: { label: 'Año', compare: (a: ContentSummary, b: ContentSummary) => (b.content.year ?? 0) - (a.content.year ?? 0) },
}
type Order = keyof typeof ORDERS

/** Una biblioteca por dentro: cada película o serie en su tarjeta, con búsqueda y orden. */
export function LibraryPage() {
  const id = Number(useParams().id)
  const { data: libraries } = useApi<Library[]>('/api/admin/libraries')
  const { data: contents, error, loading, reload } = useApi<ContentSummary[]>(`/api/admin/libraries/${id}/contents`)
  const [deleting, setDeleting] = useState<ContentSummary>()
  const [query, setQuery] = useState('')
  const [order, setOrder] = useState<Order>('title')
  const library = libraries?.find((l) => l.id === id)
  const size = contents?.reduce((sum, c) => sum + c.size, 0) ?? 0

  const shown = useMemo(() => {
    const all = contents ?? []
    const found = searchTitles(all.map((summary) => ({ title: summary.content.title, summary })), query)
    return found.map(({ summary }) => summary).sort(ORDERS[order].compare)
  }, [contents, query, order])

  return (
    <>
      <Link to="/libraries" className="btn btn-ghost btn-sm back-link">
        <ArrowLeft /> Bibliotecas
      </Link>
      <PageHeader
        title={library?.name ?? 'Biblioteca'}
        description={
          <>
            <span className="mono">{library?.dir}/</span>
            {contents && contents.length > 0 && ` · ${titles(contents.length)}`}
            {size > 0 && ` · ${formatSize(size)}`}
          </>
        }
      />

      {error && <Alert>No se pudo cargar el contenido: {error}</Alert>}
      {loading && !contents && <p className="muted">Cargando…</p>}
      {contents?.length === 0 && (
        <div className="card">
          <EmptyState icon={Clapperboard} title="Esta biblioteca está vacía">
            Lo que importes desde <Link to="/import">Importar</Link> aparece acá.
          </EmptyState>
        </div>
      )}

      {contents && contents.length > 0 && (
        <div className="library-toolbar">
          <label className="search-field">
            <Search />
            <input
              className="input"
              type="search"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder={`Buscar en ${library?.name ?? 'la biblioteca'}`}
              aria-label="Buscar por título"
            />
          </label>
          <label className="row small muted">
            Ordenar por
            <select className="select" value={order} onChange={(e) => setOrder(e.target.value as Order)}>
              {(Object.keys(ORDERS) as Order[]).map((key) => (
                <option key={key} value={key}>
                  {ORDERS[key].label}
                </option>
              ))}
            </select>
          </label>
          {query.trim() && (
            <span className="small muted">
              {shown.length} de {titles(contents.length)}
            </span>
          )}
        </div>
      )}
      {contents && contents.length > 0 && shown.length === 0 && (
        <p className="muted">Nada coincide con "{query}".</p>
      )}

      <div className="content-grid">
        {shown.map((summary) => (
          <ContentCard key={summary.content.id} summary={summary} onDelete={() => setDeleting(summary)} />
        ))}
      </div>

      {deleting && (
        <DeleteContentDialog
          summary={deleting}
          onClose={() => setDeleting(undefined)}
          onDeleted={() => {
            setDeleting(undefined)
            reload()
          }}
        />
      )}
    </>
  )
}

/**
 * Una película o serie: la carátula, qué es, cuánto tiene y si le falta algo. Lo técnico va al
 * pie, chico. Toda la tarjeta abre el detalle.
 */
function ContentCard({ summary, onDelete }: { summary: ContentSummary; onDelete: () => void }) {
  const { content } = summary
  const Icon = content.kind === 'show' ? Tv : Clapperboard
  const poster = artUrl(content.id, 'poster', content.poster_at)

  return (
    <Link to={`/libraries/${content.library_id}/contents/${content.id}`} className="card content-card">
      <div className="content-card-art">
        {poster ? <img src={poster} alt="" loading="lazy" /> : <Icon />}
      </div>
      <div className="content-card-body">
        <div className="content-card-head">
          <div className="grow">
            <strong className="content-card-title">{content.title}</strong>
            <div className="small muted">
              {[content.kind === 'show' ? 'Serie' : 'Película', content.year].filter(Boolean).join(' · ')}
            </div>
          </div>
          <button
            className="btn btn-ghost btn-sm btn-icon btn-danger"
            title={`Eliminar ${content.title}`}
            aria-label={`Eliminar ${content.title}`}
            onClick={(e) => {
              // La tarjeta es un link: el botón no tiene que abrirla.
              e.preventDefault()
              e.stopPropagation()
              onDelete()
            }}
          >
            <Trash />
          </button>
        </div>
        <div className="content-card-stats small">
          {describe(summary)}
          {summary.runtime > 0 && ` · ${formatRuntime(summary.runtime)}`}
        </div>
        <div className="content-card-notes">
          {summary.missing > 0 && (
            <span className="badge badge-warning">
              <TriangleAlert /> {summary.missing} sin archivo
            </span>
          )}
          {summary.active_jobs > 0 && (
            <span className="badge badge-accent">
              <ListOrdered /> {summary.active_jobs} en la cola
            </span>
          )}
        </div>
        <TechSummary summary={summary} />
      </div>
    </Link>
  )
}

/**
 * Lo técnico de todos sus videos, sin repetir:
 * "1080p · HEVC 10 bits · SDR · AAC 2.0 · japonés · 10 subtítulos".
 * Si los episodios no son todos iguales, aparecen las variantes ("720p, 1080p").
 */
function TechSummary({ summary }: { summary: ContentSummary }) {
  const languages = summary.audio_languages.map(languageName).filter(Boolean)
  const ranges = summary.dynamic_range.join(', ')
  const parts = [
    summary.resolutions.join(', '),
    summary.video.join(', '),
    ranges,
    summary.audio.join(', '),
    languages.join(', '),
    summary.subtitles > 0 && plural(summary.subtitles, 'subtítulo', 'subtítulos'),
  ].filter((part): part is string => Boolean(part))
  if (parts.length === 0) return null
  const hdr = summary.dynamic_range.some((range) => range !== 'SDR')
  return (
    <div className="content-card-tech">
      {parts.map((part, i) => (
        <span key={i} className={hdr && part === ranges ? 'tech-hdr' : undefined}>
          {part}
        </span>
      ))}
    </div>
  )
}

/** "1 h 52 min", "45 min": lo que dura todo. */
function formatRuntime(seconds: number) {
  const minutes = Math.round(seconds / 60)
  if (minutes < 60) return `${minutes} min`
  const hours = Math.floor(minutes / 60)
  return minutes % 60 === 0 ? `${hours} h` : `${hours} h ${minutes % 60} min`
}

/** "2 temporadas · 24 episodios · 18,4 GB", o "1 archivo · 4,1 GB" en una película. */
export function describe(summary: ContentSummary) {
  const parts =
    summary.content.kind === 'show'
      ? summary.episodes === 0
        ? ['sin episodios todavía']
        : [plural(summary.seasons, 'temporada', 'temporadas'), plural(summary.episodes, 'episodio', 'episodios')]
      : [summary.files === 0 ? 'sin archivo' : plural(summary.files, 'archivo', 'archivos')]
  if (summary.size > 0) parts.push(formatSize(summary.size))
  return parts.join(' · ')
}

function plural(count: number, one: string, many: string) {
  return `${count} ${count === 1 ? one : many}`
}

function titles(count: number) {
  return plural(count, 'título', 'títulos')
}
