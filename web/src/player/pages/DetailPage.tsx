import { useState, type ReactNode } from 'react'
import { useParams } from 'react-router'
import { Check, Play } from 'lucide-react'
import { useApi } from '../../shared/useApi'
import { formatDuration, formatWatchTime } from '../../shared/format'
import type { ContentDetail } from '../../shared/types/ContentDetail'
import type { WatchStats } from '../../shared/types/WatchStats'
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
          <WatchSummary detail={detail} />
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
                        ]}
                        watch={<EpisodeWatch episode={episode} />}
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

/** Número y título; debajo, la sinopsis (dos líneas), la duración y lo visto. */
function EpisodeInfo({
  episode,
  meta,
  watch,
}: {
  episode: ContentDetail
  meta: (string | false | null | undefined)[]
  watch?: ReactNode
}) {
  const { content } = episode
  return (
    <div className="episode-info">
      <span className="episode-heading">
        <span className="episode-number">{content.number}</span>
        <span className="episode-title">{content.title}</span>
      </span>
      {content.overview && <span className="episode-overview">{content.overview}</span>}
      <span className="meta">{meta.filter(Boolean).join(' · ')}</span>
      {watch}
    </div>
  )
}

/**
 * Lo visto de la película o de la serie entera, debajo del título: cuántas veces, cuánto tiempo
 * en total y cuándo fue la última. En una serie, lo último que se vio y cuántas veces se vio
 * completa (la menor cantidad de veces entre sus episodios).
 */
function WatchSummary({ detail }: { detail: ContentDetail }) {
  let parts: (string | undefined)[]
  if (detail.content.kind === 'movie') {
    const { watch } = detail
    if (!watch || watch.seconds < 60) return null
    parts = [
      timesLabel(watch),
      `${formatWatchTime(watch.seconds)} en total`,
      watch.last_watched_at !== null ? `la última, ${whenLabel(watch.last_watched_at)}` : undefined,
    ]
  } else {
    const episodes = detail.children.flatMap((season) =>
      season.children.map((episode) => ({ episode, label: `T${season.content.number} E${episode.content.number}` })),
    )
    const total = episodes.reduce((sum, { episode }) => sum + (episode.watch?.seconds ?? 0), 0)
    if (total < 60) return null
    const last = episodes
      .filter(({ episode }) => episode.watch?.last_watched_at != null)
      .sort((a, b) => b.episode.watch!.last_watched_at! - a.episode.watch!.last_watched_at!)[0]
    const available = episodes.filter(({ episode }) => episode.files.length > 0)
    const complete = available.length > 0 ? Math.min(...available.map(({ episode }) => wholeTimes(episode.watch))) : 0
    parts = [
      complete === 1 ? 'Vista completa 1 vez' : complete > 1 ? `Vista completa ${complete} veces` : undefined,
      `${formatWatchTime(total)} vistos en total`,
      last ? `lo último: ${last.label}, ${whenLabel(last.episode.watch!.last_watched_at!)}` : undefined,
    ]
  }
  return <p className="meta watch-summary">{parts.filter(Boolean).join(' · ')}</p>
}

/**
 * Lo visto de un episodio, en su propia línea: "Visto 3 veces", el tiempo reproducido en total y
 * cuándo fue la última. Si todavía no llega a una vez completa, igual dice cuánto se vio
 * ("Empezado, 4 min vistos"); con menos de un minuto, nada (abrirlo y cerrarlo no es verlo).
 */
function EpisodeWatch({ episode }: { episode: ContentDetail }) {
  const { watch, progress } = episode
  const seconds = watch?.seconds ?? 0
  const times = timesLabel(watch) ?? (progress?.watched ? 'Visto' : seconds >= 60 ? 'Empezado' : undefined)
  if (!times) return null
  const total = seconds >= 60 ? `${formatWatchTime(seconds)} ${times === 'Empezado' ? 'vistos' : 'en total'}` : undefined
  const last = watch?.last_watched_at != null ? `la última, ${whenLabel(watch.last_watched_at)}` : undefined
  return (
    <span className="episode-watch">
      <strong>{times}</strong>
      {[total, last].filter(Boolean).join(', ')}
    </span>
  )
}

/** Las visualizaciones completas a las que equivale lo reproducido, redondeadas (de 0,9 en adelante, una). */
function wholeTimes(watch: WatchStats | null) {
  const times = watch?.times ?? 0
  return times < 0.9 ? 0 : Math.max(1, Math.round(times))
}

/** "Visto 1 vez", "Visto 12 veces", o nada si todavía no equivale a una vez completa. */
function timesLabel(watch: WatchStats | null) {
  const times = wholeTimes(watch)
  if (times === 0) return undefined
  return times === 1 ? 'Visto 1 vez' : `Visto ${times} veces`
}

/** "hoy", "ayer", "hace 3 días", "el 12 de marzo" o, si no es de este año, "el 12 de marzo de 2024". */
function whenLabel(seconds: number) {
  const date = new Date(seconds * 1000)
  const now = new Date()
  const midnight = (d: Date) => new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime()
  const days = Math.round((midnight(now) - midnight(date)) / 86_400_000)
  if (days === 0) return 'hoy'
  if (days === 1) return 'ayer'
  if (days < 7) return `hace ${days} días`
  const year = date.getFullYear() === now.getFullYear() ? undefined : 'numeric'
  return `el ${date.toLocaleDateString('es-AR', { day: 'numeric', month: 'long', year })}`
}

function duration(detail: ContentDetail) {
  const file = pickFile(detail.files)
  return file ? formatDuration(file.duration) : null
}

function seasonCount(count: number) {
  return count === 1 ? '1 temporada' : `${count} temporadas`
}
