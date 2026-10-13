import { useState } from 'react'
import { Link, useParams } from 'react-router'
import { ArrowLeft, Captions, Clapperboard, ListOrdered, Trash, TriangleAlert, Tv } from 'lucide-react'
import { useApi } from '../../shared/useApi'
import { formatSize } from '../../shared/format'
import { languageName } from '../../shared/languages'
import type { ContentSummary } from '../../shared/types/ContentSummary'
import type { Library } from '../../shared/types/Library'
import { Alert, EmptyState, PageHeader } from '../ui'
import { DeleteContentDialog } from '../DeleteContentDialog'

/** Una biblioteca por dentro: cada película o serie en su tarjeta. */
export function LibraryPage() {
  const id = Number(useParams().id)
  const { data: libraries } = useApi<Library[]>('/api/admin/libraries')
  const { data: contents, error, loading, reload } = useApi<ContentSummary[]>(`/api/admin/libraries/${id}/contents`)
  const [deleting, setDeleting] = useState<ContentSummary>()
  const library = libraries?.find((l) => l.id === id)
  const size = contents?.reduce((sum, c) => sum + c.size, 0) ?? 0

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

      <div className="content-grid">
        {contents?.map((summary) => (
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

/** Una película o serie: qué es, cuánto tiene y si le falta algo. Toda la tarjeta abre el detalle. */
function ContentCard({ summary, onDelete }: { summary: ContentSummary; onDelete: () => void }) {
  const { content } = summary
  const Icon = content.kind === 'show' ? Tv : Clapperboard

  return (
    <Link to={`/libraries/${content.library_id}/contents/${content.id}`} className="card content-card">
      <div className="content-card-head">
        <Icon className="content-card-icon" />
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
      <TechSummary summary={summary} />
      <div className="content-card-notes">
        {summary.subtitles > 0 && (
          <span className="badge">
            <Captions /> {summary.subtitles} subtítulo{summary.subtitles === 1 ? '' : 's'}
          </span>
        )}
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
    </Link>
  )
}

/**
 * Lo técnico de todos sus videos, sin repetir: "1080p · HEVC 10 bits · SDR · AAC 2.0 · japonés".
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
  ].filter(Boolean)
  if (parts.length === 0) return null
  const hdr = summary.dynamic_range.some((range) => range !== 'SDR')
  return (
    <div className="content-card-tech small">
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
