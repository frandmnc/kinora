import { useState } from 'react'
import { Link, useNavigate, useParams } from 'react-router'
import { ArrowLeft, Trash } from 'lucide-react'
import { artUrl } from '../../shared/api'
import { useApi } from '../../shared/useApi'
import { formatSize } from '../../shared/format'
import type { ContentDetail } from '../../shared/types/ContentDetail'
import type { ContentSummary } from '../../shared/types/ContentSummary'
import type { Library } from '../../shared/types/Library'
import { Alert, Card, PageHeader } from '../ui'
import { Files } from '../ContentFiles'
import { DeleteContentDialog } from '../DeleteContentDialog'
import { MetadataCard } from '../MetadataCard'
import { describe } from './LibraryPage'

/** Una película o serie: sus archivos (o temporadas y episodios), versiones y subtítulos. */
export function ContentPage() {
  const params = useParams()
  const libraryId = Number(params.libraryId)
  const id = Number(params.id)
  const navigate = useNavigate()
  const { data: libraries } = useApi<Library[]>('/api/admin/libraries')
  const { data: detail, error, reload } = useApi<ContentDetail>(`/api/contents/${id}`)
  // Los números (archivos, tamaño, cola) salen del resumen de la biblioteca.
  const { data: summaries, reload: reloadSummary } = useApi<ContentSummary[]>(
    `/api/admin/libraries/${libraryId}/contents`,
  )
  const [deleting, setDeleting] = useState(false)
  const library = libraries?.find((l) => l.id === libraryId)
  const summary = summaries?.find((s) => s.content.id === id)

  function refresh() {
    reload()
    reloadSummary()
  }

  return (
    <>
      <Link to={`/libraries/${libraryId}`} className="btn btn-ghost btn-sm back-link">
        <ArrowLeft /> {library?.name ?? 'Biblioteca'}
      </Link>
      {error && <Alert>No se pudo cargar: {error}</Alert>}
      {detail && (
        <>
          <PageHeader
            title={detail.content.title}
            description={
              <>
                {[detail.content.kind === 'show' ? 'Serie' : 'Película', detail.content.year].filter(Boolean).join(' · ')}
                {summary && ` · ${describe(summary)}`}
                {detail.content.dir && (
                  <>
                    {' · '}
                    <span className="mono">{detail.content.dir}/</span>
                  </>
                )}
              </>
            }
            actions={
              summary && (
                <button className="btn btn-danger" onClick={() => setDeleting(true)}>
                  <Trash /> Eliminar
                </button>
              )
            }
          />

          <MetadataCard content={detail.content} onChange={refresh} />

          {detail.content.kind === 'movie' ? (
            <Card title="Archivos">
              <div className="card-body">
                <Files node={detail} onChange={refresh} />
              </div>
            </Card>
          ) : detail.children.length === 0 ? (
            <Alert tone="info">Esta serie todavía no tiene temporadas.</Alert>
          ) : (
            detail.children.map((season) => <SeasonCard key={season.content.id} season={season} onChange={refresh} />)
          )}
        </>
      )}

      {deleting && summary && (
        <DeleteContentDialog
          summary={summary}
          onClose={() => setDeleting(false)}
          onDeleted={() => navigate(`/libraries/${libraryId}`, { replace: true })}
        />
      )}
    </>
  )
}

/** Una temporada en su propia tarjeta, con sus episodios uno debajo del otro. */
function SeasonCard({ season, onChange }: { season: ContentDetail; onChange: () => void }) {
  const files = season.children.flatMap((episode) => episode.files)
  const size = files.reduce((sum, file) => sum + file.size, 0)
  const missing = season.children.filter((episode) => episode.files.length === 0).length
  const count = season.children.length

  return (
    <Card
      title={season.content.title}
      description={[
        `${count} episodio${count === 1 ? '' : 's'}`,
        size > 0 && formatSize(size),
        missing > 0 && `${missing} sin archivo`,
      ]
        .filter(Boolean)
        .join(' · ')}
    >
      {season.children.map((episode) => {
        const number = `E${String(episode.content.number ?? 0).padStart(2, '0')}`
        const still = artUrl(episode.content.id, 'still', episode.content.still_at)
        return (
          <div key={episode.content.id} className="episode-row">
            <div className="episode-still">{still ? <img src={still} alt="" loading="lazy" /> : number}</div>
            <div className="episode-row-main">
              <div className="episode-row-title">
                <span className="episode-row-number">{number}</span>
                {episode.content.title}
              </div>
              {episode.content.overview && <p className="episode-row-overview">{episode.content.overview}</p>}
              <Files node={episode} onChange={onChange} />
            </div>
          </div>
        )
      })}
    </Card>
  )
}
