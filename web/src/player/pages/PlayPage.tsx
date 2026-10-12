import { useCallback, useEffect, useRef, useState } from 'react'
import { useLocation, useNavigate, useParams } from 'react-router'
import { api, mediaUrl } from '../../shared/api'
import { useApi } from '../../shared/useApi'
import { formatDuration } from '../../shared/format'
import type { ContentDetail } from '../../shared/types/ContentDetail'
import type { MediaFile } from '../../shared/types/MediaFile'
import type { PlaybackState } from '../../shared/types/PlaybackState'
import type { PlaybackReport } from '../../shared/types/PlaybackReport'
import { Message } from './HomePage'

/** Cada cuánto el Player le avisa al servidor dónde está. */
const REPORT_EVERY_MS = 10_000

/** Reproductor a pantalla completa. Por ahora usa los controles nativos del navegador. */
export function PlayPage() {
  const { id } = useParams()
  const navigate = useNavigate()
  const location = useLocation()
  const { data: detail, error } = useApi<ContentDetail>(`/api/contents/${id}`)
  const [tryAnyway, setTryAnyway] = useState(false)

  // Escape o Backspace vuelven atrás, como la tecla "Atrás" del control de la TV.
  useEffect(() => {
    function onKey(event: KeyboardEvent) {
      if (event.key === 'Escape' || event.key === 'Backspace') navigate(-1)
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [navigate])

  if (error) return <Message title="No se pudo cargar" detail={error} />
  if (!detail) return <Message title="Cargando…" />

  const file = detail.files[0]
  if (!file) return <Message title="Sin archivo para reproducir" />

  const title: string = location.state?.title ?? detail.content.title

  // Preguntarle al navegador si entiende el códec antes de intentar reproducir.
  const mimeType = `video/mp4; codecs="${file.video_codec}"`
  const supported = !file.video_codec || document.createElement('video').canPlayType(mimeType) !== ''

  return (
    <div className="player">
      <button className="player-back" onClick={() => navigate(-1)}>
        ← {title}
      </button>
      {supported || tryAnyway ? (
        <Video detail={detail} file={file} />
      ) : (
        <div className="message">
          <h1>Este navegador no reproduce este video</h1>
          <p>
            Códec: <code>{file.video_codec}</code>
          </p>
          <button className="play-button" onClick={() => setTryAnyway(true)}>
            Intentar igual
          </button>
        </div>
      )}
    </div>
  )
}

/** El `<video>` con los avisos de progreso al servidor y la reanudación. */
function Video({ detail, file }: { detail: ContentDetail; file: MediaFile }) {
  const contentId = detail.content.id
  const videoRef = useRef<HTMLVideoElement>(null)
  // Última posición conocida. Se guarda aparte porque al cerrar el reproductor
  // el `<video>` ya no existe y hay que mandar igual el último aviso.
  const last = useRef({ position: 0, duration: null as number | null })
  const [resumedAt, setResumedAt] = useState<number>()

  const report = useCallback(
    (state: PlaybackState) => {
      const video = videoRef.current
      if (video) {
        last.current = {
          position: video.currentTime,
          duration: Number.isFinite(video.duration) ? video.duration : null,
        }
      }
      const body: PlaybackReport = { ...last.current, state }
      api(`/api/contents/${contentId}/progress`, { method: 'PUT', body, keepalive: true }).catch(() => {
        // Un aviso perdido no importa: el siguiente llega en 10 segundos.
      })
    },
    [contentId],
  )

  // Aviso periódico, y aviso final al salir del reproductor o cerrar la pestaña.
  useEffect(() => {
    const currentState = (): PlaybackState => (videoRef.current?.paused ? 'paused' : 'playing')
    const timer = setInterval(() => report(currentState()), REPORT_EVERY_MS)
    const onPageHide = () => report('stopped')
    window.addEventListener('pagehide', onPageHide)
    return () => {
      clearInterval(timer)
      window.removeEventListener('pagehide', onPageHide)
      report('stopped')
    }
  }, [report])

  // Retomar donde quedó, salvo que ya lo haya terminado o esté al principio o al final.
  function resume(video: HTMLVideoElement) {
    const progress = detail.progress
    if (progress && !progress.watched && progress.position > 10 && progress.position < video.duration - 30) {
      video.currentTime = progress.position
      setResumedAt(progress.position)
      setTimeout(() => setResumedAt(undefined), 4000)
    }
  }

  return (
    <>
      <video
        ref={videoRef}
        src={mediaUrl(`/api/files/${file.id}/stream`)}
        controls
        autoPlay
        onLoadedMetadata={(e) => resume(e.currentTarget)}
        onPlay={() => report('playing')}
        onPause={() => report('paused')}
        onSeeked={(e) => report(e.currentTarget.paused ? 'paused' : 'playing')}
      />
      {resumedAt !== undefined && <div className="player-toast">Continuando desde {formatDuration(resumedAt)}</div>}
    </>
  )
}
