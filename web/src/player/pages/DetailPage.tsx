import { useState } from 'react'
import { useParams } from 'react-router'
import { Check, Play } from 'lucide-react'
import { useApi } from '../../shared/useApi'
import { formatDuration } from '../../shared/format'
import type { ContentDetail } from '../../shared/types/ContentDetail'
import { pickFile, versionLabel } from '../../shared/versions'
import { artUrl } from '../../shared/api'
import { Poster } from '../Poster'
import { FocusGroup, FocusableButton, FocusableLink } from '../focus'
import { useHighlightLibrary } from '../Sidebar'
import { Message } from './HomePage'

/** Detalle de una película o una serie. En una serie lista temporadas y episodios. */
export function DetailPage() {
  const { id } = useParams()
  const { data: detail, error } = useApi<ContentDetail>(`/api/contents/${id}`)
  // La versión elegida (si hay varias); si no se eligió, la de mayor resolución.
  // Los hooks van antes de cualquier `return`: React los identifica por el orden en que se llaman.
  const [chosen, setChosen] = useState<number>()
  useHighlightLibrary(detail?.content.library_id)

  if (error) return <Message title="No se pudo cargar" detail={error} />
  if (!detail) return <Message title="Cargando…" />

  const { content } = detail
  const seasons = detail.children
  const next = content.kind === 'show' ? nextEpisode(detail) : undefined
  const playable = content.kind === 'movie' ? (detail.files.length > 0 ? detail : undefined) : next?.episode
  const version = playable && pickFile(playable.files, chosen)
  const versionQuery = playable && playable.files.length > 1 && version ? `?file=${version.id}` : ''

  return (
    // El foco arranca en el botón de reproducir.
    <FocusGroup className="screen detail" autoFocus preferredChildFocusKey={playable ? 'PLAY' : undefined}>
      {/* El fondo, si hay, detrás del encabezado y fundido con la pantalla. */}
      {content.backdrop_at && (
        <div
          className="detail-backdrop"
          style={{ backgroundImage: `url(${artUrl(content.id, 'backdrop', content.backdrop_at)})` }}
          aria-hidden="true"
        />
      )}
      <div className="detail-header">
        <Poster title={content.title} src={artUrl(content.id, 'poster', content.poster_at)} />
        <div>
          <h1>{content.title}</h1>
          <p className="meta">
            {[content.year, content.kind === 'show' ? seasonCount(seasons.length) : duration(detail)]
              .filter(Boolean)
              .join(' · ')}
          </p>
          {content.overview && <p className="overview">{content.overview}</p>}
          {playable ? (
            <>
              <FocusableLink
                focusKey="PLAY"
                to={`/play/${playable.content.id}${versionQuery}`}
                state={{ title: next ? `${content.title} · ${next.label}` : content.title }}
                className="play-button"
              >
                <Play size={22} fill="currentColor" />
                {playLabel(playable, next?.label)}
              </FocusableLink>
              {playable.files.length > 1 && version && (
                <FocusGroup className="versions">
                  <span className="meta">Versión</span>
                  {playable.files.map((file) => (
                    <FocusableButton
                      key={file.id}
                      className={`chip ${file.id === version.id ? 'selected' : ''}`}
                      pressed={file.id === version.id}
                      onPress={() => setChosen(file.id)}
                    >
                      {versionLabel(file)}
                    </FocusableButton>
                  ))}
                </FocusGroup>
              )}
            </>
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
                      <EpisodeStill episode={episode} />
                      <EpisodeInfo
                        episode={episode}
                        meta={[
                          duration(episode),
                          episode.files.length > 1 && `${episode.files.length} versiones`,
                          episode.progress?.watched && 'Visto',
                        ]}
                      />
                    </FocusableLink>
                  ) : (
                    <div className="episode unavailable">
                      <EpisodeStill episode={episode} />
                      <EpisodeInfo episode={episode} meta={['sin archivo']} />
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

/**
 * Un cuadro del episodio (o su número, si no hay), con una barra abajo si se empezó a ver y una
 * marca si ya se vio.
 */
function EpisodeStill({ episode }: { episode: ContentDetail }) {
  const { content, progress } = episode
  const still = artUrl(content.id, 'still', content.still_at)
  const percent =
    progress && !progress.watched && progress.duration
      ? Math.min(100, (progress.position / progress.duration) * 100)
      : 0
  return (
    <div className="episode-still">
      {still ? <img src={still} alt="" loading="lazy" /> : <span>{content.number}</span>}
      {percent > 0 && (
        <div className="episode-progress" title={formatDuration(progress!.position)}>
          <div style={{ width: `${percent}%` }} />
        </div>
      )}
      {progress?.watched && (
        <span className="episode-watched" aria-label="Visto">
          <Check />
        </span>
      )}
    </div>
  )
}

/** Número y título; debajo, la sinopsis (dos líneas) y la duración. */
function EpisodeInfo({ episode, meta }: { episode: ContentDetail; meta: (string | false | null | undefined)[] }) {
  const { content } = episode
  return (
    <div className="episode-info">
      <span className="episode-heading">
        <span className="episode-number">{content.number}</span>
        <span className="episode-title">{content.title}</span>
      </span>
      {content.overview && <span className="episode-overview">{content.overview}</span>}
      <span className="meta">{meta.filter(Boolean).join(' · ')}</span>
    </div>
  )
}

function duration(detail: ContentDetail) {
  const file = pickFile(detail.files)
  return file ? formatDuration(file.duration) : null
}

function seasonCount(count: number) {
  return count === 1 ? '1 temporada' : `${count} temporadas`
}
