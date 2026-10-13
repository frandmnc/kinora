import { useState } from 'react'
import { useParams } from 'react-router'
import { useFocusable } from '@noriginmedia/norigin-spatial-navigation'
import { ArrowLeft, Play } from 'lucide-react'
import { useApi } from '../../shared/useApi'
import { formatDuration } from '../../shared/format'
import type { ContentDetail } from '../../shared/types/ContentDetail'
import type { MediaFile } from '../../shared/types/MediaFile'
import { pickFile, versionLabel } from '../../shared/versions'
import { Poster } from '../Poster'
import { FocusGroup, FocusableLink } from '../focus'
import { Message } from './HomePage'

/** Detalle de una película o una serie. En una serie lista temporadas y episodios. */
export function DetailPage() {
  const { id } = useParams()
  const { data: detail, error } = useApi<ContentDetail>(`/api/contents/${id}`)
  // La versión elegida (si hay varias); si no se eligió, la de mayor resolución.
  // Los hooks van antes de cualquier `return`: React los identifica por el orden en que se llaman.
  const [chosen, setChosen] = useState<number>()

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
                    <VersionChip
                      key={file.id}
                      file={file}
                      selected={file.id === version.id}
                      onSelect={() => setChosen(file.id)}
                    />
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
                      <span className="episode-number">{episode.content.number}</span>
                      <span className="episode-title">{episode.content.title}</span>
                      <ProgressMark detail={episode} />
                      {episode.files.length > 1 && <span className="meta">{episode.files.length} versiones</span>}
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

/** Una versión para elegir: Enter (o un clic) la deja elegida para el botón de reproducir. */
function VersionChip({ file, selected, onSelect }: { file: MediaFile; selected: boolean; onSelect: () => void }) {
  const { ref, focused, focusSelf } = useFocusable<object, HTMLButtonElement>({ onEnterPress: onSelect })
  return (
    <button
      ref={ref}
      className={`version-chip ${selected ? 'selected' : ''} ${focused ? 'is-focused' : ''}`}
      onClick={onSelect}
      onMouseEnter={() => focusSelf()}
      aria-pressed={selected}
    >
      {versionLabel(file)}
    </button>
  )
}

function duration(detail: ContentDetail) {
  const file = pickFile(detail.files)
  return file ? formatDuration(file.duration) : null
}

function seasonCount(count: number) {
  return count === 1 ? '1 temporada' : `${count} temporadas`
}
