import { useParams } from 'react-router'
import { ArrowLeft, Play } from 'lucide-react'
import { useApi } from '../../shared/useApi'
import { formatDuration } from '../../shared/format'
import type { ContentDetail } from '../../shared/types/ContentDetail'
import { Poster } from '../Poster'
import { FocusGroup, FocusableLink } from '../focus'
import { Message } from './HomePage'

/** Detalle de una película o una serie. En una serie lista temporadas y episodios. */
export function DetailPage() {
  const { id } = useParams()
  const { data: detail, error } = useApi<ContentDetail>(`/api/contents/${id}`)

  if (error) return <Message title="No se pudo cargar" detail={error} />
  if (!detail) return <Message title="Cargando…" />

  const { content } = detail
  const seasons = detail.children
  const next = content.kind === 'show' ? nextEpisode(detail) : undefined
  const playable = content.kind === 'movie' ? (detail.files.length > 0 ? detail : undefined) : next?.episode

  return (
    // El foco arranca en el botón de reproducir.
    <FocusGroup className="screen" autoFocus preferredChildFocusKey={playable ? 'PLAY' : undefined}>
      <FocusableLink to="/" className="back">
        <ArrowLeft size={20} /> Inicio
      </FocusableLink>

      <div className="detail-header">
        <Poster title={content.title} />
        <div>
          <h1>{content.title}</h1>
          <p className="meta">
            {[content.year, content.kind === 'show' ? seasonCount(seasons.length) : duration(detail)]
              .filter(Boolean)
              .join(' · ')}
          </p>
          {content.overview && <p className="overview">{content.overview}</p>}
          {playable ? (
            <FocusableLink
              focusKey="PLAY"
              to={`/play/${playable.content.id}`}
              state={{ title: next ? `${content.title} · ${next.label}` : content.title }}
              className="play-button"
            >
              <Play size={22} fill="currentColor" />
              {playLabel(playable, next?.label)}
            </FocusableLink>
          ) : (
            <p className="meta">Sin archivo para reproducir.</p>
          )}
        </div>
      </div>

      {seasons.map((season) => (
        <section key={season.content.id} className="season">
          {/* Con una sola temporada, el nivel de temporada no aporta: se oculta. */}
          {seasons.length > 1 && <h2>{season.content.title}</h2>}
          <ol className="episodes">
            {season.children.map((episode) => {
              const title = `${content.title} · T${season.content.number} E${episode.content.number}`
              return (
                <li key={episode.content.id}>
                  {episode.files.length > 0 ? (
                    <FocusableLink to={`/play/${episode.content.id}`} state={{ title }} className="episode">
                      <span className="episode-number">{episode.content.number}</span>
                      <span className="episode-title">{episode.content.title}</span>
                      <ProgressMark detail={episode} />
                      <span className="meta">{duration(episode)}</span>
                    </FocusableLink>
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
    </FocusGroup>
  )
}

/** El episodio a reproducir: el primero (en orden) que no esté visto; si todos lo están, el primero. */
function nextEpisode(show: ContentDetail) {
  const episodes = show.children.flatMap((season) =>
    season.children
      .filter((episode) => episode.files.length > 0)
      .map((episode) => ({ episode, label: `T${season.content.number} E${episode.content.number}` })),
  )
  return episodes.find(({ episode }) => !episode.progress?.watched) ?? episodes[0]
}

function playLabel(playable: ContentDetail, episodeLabel?: string) {
  const progress = playable.progress
  const resuming = progress && !progress.watched && progress.position > 10
  const action = resuming ? 'Continuar' : 'Reproducir'
  const what = episodeLabel ? ` ${episodeLabel}` : ''
  const at = resuming ? ` (${formatDuration(progress.position)})` : ''
  return `${action}${what}${at}`
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
