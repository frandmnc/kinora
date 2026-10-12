import { Link, useParams } from 'react-router'
import { useApi } from '../../shared/useApi'
import { formatDuration } from '../../shared/format'
import type { ContentDetail } from '../../shared/types/ContentDetail'
import { Poster } from '../Poster'
import { Message } from './HomePage'

/** Detalle de una película o una serie. En una serie lista temporadas y episodios. */
export function DetailPage() {
  const { id } = useParams()
  const { data: detail, error } = useApi<ContentDetail>(`/api/contents/${id}`)

  if (error) return <Message title="No se pudo cargar" detail={error} />
  if (!detail) return <Message title="Cargando…" />

  const { content } = detail
  const seasons = detail.children

  return (
    <div className="screen">
      <Link to="/" className="back">
        ← Inicio
      </Link>
      <div className="detail-header">
        <Poster content={content} />
        <div>
          <h1>{content.title}</h1>
          <p className="meta">
            {[content.year, content.kind === 'show' ? seasonCount(seasons.length) : duration(detail)]
              .filter(Boolean)
              .join(' · ')}
          </p>
          {content.overview && <p className="overview">{content.overview}</p>}
          {content.kind === 'movie' &&
            (detail.files.length > 0 ? (
              <Link to={`/play/${content.id}`} state={{ title: content.title }} className="play-button" autoFocus>
                {detail.progress && !detail.progress.watched
                  ? `▶ Continuar (${formatDuration(detail.progress.position)})`
                  : '▶ Reproducir'}
              </Link>
            ) : (
              <p className="meta">Sin archivo para reproducir.</p>
            ))}
        </div>
      </div>

      {seasons.map((season) => (
        <section key={season.content.id} className="season">
          {/* Con una sola temporada, el nivel de temporada no aporta: se oculta. */}
          {seasons.length > 1 && <h2>{season.content.title}</h2>}
          <ol className="episodes">
            {season.children.map((episode) => {
              const title = `${content.title} · T${season.content.number} E${episode.content.number}`
              const playable = episode.files.length > 0
              return (
                <li key={episode.content.id}>
                  {playable ? (
                    <Link to={`/play/${episode.content.id}`} state={{ title }} className="episode">
                      <span className="episode-number">{episode.content.number}</span>
                      <span className="episode-title">{episode.content.title}</span>
                      <ProgressMark detail={episode} />
                      <span className="meta">{duration(episode)}</span>
                    </Link>
                  ) : (
                    <div className="episode unavailable">
                      <span className="episode-number">{episode.content.number}</span>
                      <span className="episode-title">{episode.content.title}</span>
                      <span className="meta">sin archivo</span>
                    </div>
                  )}
                </li>
              )
            })}
          </ol>
        </section>
      ))}
    </div>
  )
}

/** "✓ Visto", una barra con el avance, o nada si no se empezó. */
function ProgressMark({ detail }: { detail: ContentDetail }) {
  const progress = detail.progress
  if (!progress) return null
  if (progress.watched) return <span className="watched">✓ Visto</span>
  const percent = progress.duration ? Math.min(100, (progress.position / progress.duration) * 100) : 0
  return (
    <span className="episode-progress" title={formatDuration(progress.position)}>
      <div style={{ width: `${percent}%` }} />
    </span>
  )
}

function duration(detail: ContentDetail) {
  const file = detail.files[0]
  return file ? formatDuration(file.duration) : null
}

function seasonCount(count: number) {
  return count === 1 ? '1 temporada' : `${count} temporadas`
}
