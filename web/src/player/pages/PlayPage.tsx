import { useCallback, useEffect, useRef, useState, type MouseEvent } from 'react'
import { useLocation, useNavigate, useParams } from 'react-router'
import { pickFile } from '../../shared/versions'
import { pause as pauseNavigation, resume as resumeNavigation } from '@noriginmedia/norigin-spatial-navigation'
import {
  ArrowLeft,
  Captions,
  Check,
  LoaderCircle,
  Maximize,
  Minimize,
  Pause,
  Play,
  RotateCcw,
  SkipForward,
  Volume2,
  VolumeX,
  type LucideIcon,
} from 'lucide-react'
import { api, mediaUrl } from '../../shared/api'
import { useApi } from '../../shared/useApi'
import { formatDuration } from '../../shared/format'
import type { ContentDetail } from '../../shared/types/ContentDetail'
import type { MediaFile } from '../../shared/types/MediaFile'
import type { NextUp } from '../../shared/types/NextUp'
import type { PlaybackState } from '../../shared/types/PlaybackState'
import type { PlaybackReport } from '../../shared/types/PlaybackReport'
import type { SubtitleTrack } from '../../shared/types/SubtitleTrack'
import { isBackKey } from '../App'
import { Message } from './HomePage'

/** Cada cuánto el Player le avisa al servidor dónde está. */
const REPORT_EVERY_MS = 10_000
/** Cuánto saltan las flechas izquierda y derecha. */
const SEEK_SECONDS = 10
/** Mientras se reproduce, los controles se esconden después de este tiempo sin tocar nada. */
const HIDE_CONTROLS_MS = 3000
/** Lo mismo, pero eligiendo un botón de la barra con el control remoto: se da más tiempo. */
const HIDE_BAR_MS = 8000
/** Cuánto tiene que tardar una carga para que se muestre el spinner. */
const SPINNER_DELAY_MS = 150
/** Dónde se recuerda el idioma de subtítulos elegido (o "off"), para los próximos videos. */
const SUBTITLE_PREFERENCE = 'kinora.subtitles'
// Dónde termina el subtítulo, en % de la altura desde arriba: sin controles y con controles.
const CUE_LINE = 90
const CUE_LINE_WITH_CONTROLS = 80

function readPreference(): string | null {
  try {
    return localStorage.getItem(SUBTITLE_PREFERENCE)
  } catch {
    return null
  }
}

function savePreference(value: string) {
  try {
    localStorage.setItem(SUBTITLE_PREFERENCE, value)
  } catch {
    // Sin almacenamiento (modo privado): la elección vale solo para este video.
  }
}

/** La pista con la que arranca: la del idioma elegido la última vez, o la marcada por defecto. */
function initialTrack(tracks: SubtitleTrack[]): number | null {
  const preference = readPreference()
  if (preference === 'off') return null
  const preferred = preference ? tracks.find((t) => t.srclang === preference && !t.forced) : undefined
  return (preferred ?? tracks.find((t) => t.is_default))?.id ?? null
}

/** Reproductor a pantalla completa con controles propios, pensados para el control remoto. */
export function PlayPage() {
  const { id } = useParams()
  const location = useLocation()
  const { data: detail, error } = useApi<ContentDetail>(`/api/contents/${id}`)
  const [tryAnyway, setTryAnyway] = useState(false)

  // En el reproductor las flechas adelantan y atrasan: la navegación entre elementos se apaga.
  useEffect(() => {
    pauseNavigation()
    return () => resumeNavigation()
  }, [])

  if (error) return <Message title="No se pudo cargar" detail={error} />
  if (!detail) return <Message title="Cargando…" />

  // `?file=` elige una versión; si no viene, la de mayor resolución.
  const requested = Number(new URLSearchParams(location.search).get('file')) || null
  const file = pickFile(detail.files, requested)
  if (!file) return <Message title="Sin archivo para reproducir" />

  const title: string = location.state?.title ?? detail.content.title

  // Preguntarle al navegador si entiende el códec antes de intentar reproducir.
  const mimeType = `video/mp4; codecs="${file.video_codec}"`
  const supported = !file.video_codec || document.createElement('video').canPlayType(mimeType) !== ''

  if (!supported && !tryAnyway) {
    return (
      <div className="message">
        <h1>Este navegador no reproduce este video</h1>
        <p>
          Códec: <code>{file.video_codec}</code>
        </p>
        <button className="play-button" onClick={() => setTryAnyway(true)} autoFocus>
          Intentar igual
        </button>
      </div>
    )
  }
  // `key`: al pasar de un video a otro, el reproductor se crea de nuevo. Si se reutilizara,
  // el último aviso del video anterior saldría con la posición del nuevo (y pisaría su progreso).
  return <Video key={detail.content.id} detail={detail} file={file} title={title} />
}

/** Un botón de la barra del reproductor. */
type BarControl = { id: string; icon: LucideIcon; label: string; onPress: () => void; filled?: boolean; active?: boolean }

/** El `<video>` con controles propios, los avisos de progreso al servidor y la reanudación. */
function Video({ detail, file, title }: { detail: ContentDetail; file: MediaFile; title: string }) {
  const navigate = useNavigate()
  const contentId = detail.content.id
  const containerRef = useRef<HTMLDivElement>(null)
  const videoRef = useRef<HTMLVideoElement>(null)
  // Última posición conocida. Se guarda aparte porque al cerrar el reproductor
  // el `<video>` ya no existe y hay que mandar igual el último aviso.
  const last = useRef({ position: 0, duration: null as number | null })
  const hideTimer = useRef<number>(undefined)
  const flashTimer = useRef<number>(undefined)

  const [playing, setPlaying] = useState(false)
  const [time, setTime] = useState(0)
  const [duration, setDuration] = useState(0)
  const [controlsVisible, setControlsVisible] = useState(true)
  const [fullscreen, setFullscreen] = useState(false)
  const [muted, setMuted] = useState(false)
  const [flash, setFlash] = useState<string>()
  const [buffering, setBuffering] = useState(true)
  const [showSpinner, setShowSpinner] = useState(false)

  // El spinner aparece solo si la carga tarda: si es instantánea, mostrarlo sería un parpadeo.
  useEffect(() => {
    if (!buffering) {
      setShowSpinner(false)
      return
    }
    const timer = window.setTimeout(() => setShowSpinner(true), SPINNER_DELAY_MS)
    return () => window.clearTimeout(timer)
  }, [buffering])

  // Subtítulos de este archivo. `menu` es la fila resaltada del menú (0 = desactivados), o null si está cerrado.
  const tracks = detail.subtitles.filter((track) => track.media_file_id === file.id)
  const [subtitle, setSubtitle] = useState<number | null>(() => initialTrack(tracks))
  const [menu, setMenu] = useState<number | null>(null)
  // El botón de la barra elegido con las flechas (Arriba entra a la barra), o null fuera de ella.
  const [bar, setBar] = useState<number | null>(null)
  const inBar = useRef(false)
  inBar.current = bar !== null

  // En un episodio, el que sigue (para "Siguiente episodio"); en una película no se pide.
  const { data: nextUp } = useApi<NextUp | null>(
    detail.content.kind === 'episode' ? `/api/contents/${contentId}/next` : '',
  )

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

  /** Muestra los controles; si se está reproduciendo, se esconden solos después de un rato. */
  const showControls = useCallback(() => {
    setControlsVisible(true)
    window.clearTimeout(hideTimer.current)
    hideTimer.current = window.setTimeout(
      () => {
        if (videoRef.current?.paused) return
        setControlsVisible(false)
        setBar(null)
      },
      inBar.current ? HIDE_BAR_MS : HIDE_CONTROLS_MS,
    )
  }, [])

  const showFlash = useCallback((text: string, ms = 900) => {
    setFlash(text)
    window.clearTimeout(flashTimer.current)
    flashTimer.current = window.setTimeout(() => setFlash(undefined), ms)
  }, [])

  const togglePlay = useCallback(() => {
    const video = videoRef.current
    if (!video) return
    if (video.paused) video.play()
    else video.pause()
    showControls()
  }, [showControls])

  const seekBy = useCallback(
    (seconds: number) => {
      const video = videoRef.current
      if (!video || !Number.isFinite(video.duration)) return
      video.currentTime = Math.min(Math.max(video.currentTime + seconds, 0), video.duration - 1)
      showFlash(seconds > 0 ? `+${seconds} s` : `−${-seconds} s`)
      showControls()
    },
    [showControls, showFlash],
  )

  const toggleMute = useCallback(() => {
    const video = videoRef.current
    if (!video) return
    video.muted = !video.muted
    showFlash(video.muted ? 'Sin sonido' : 'Con sonido')
    showControls()
  }, [showControls, showFlash])

  /** Vuelve al principio del capítulo o la película, y sigue reproduciendo. */
  const restart = useCallback(() => {
    const video = videoRef.current
    if (!video) return
    video.currentTime = 0
    video.play()
    showFlash('Desde el principio')
    showControls()
  }, [showControls, showFlash])

  /** Pasa al episodio siguiente. Reemplaza al actual en el historial: Atrás vuelve al detalle. */
  const playNext = useCallback(() => {
    if (!nextUp) return
    navigate(`/play/${nextUp.content_id}`, { replace: true, state: { title: nextUp.title } })
  }, [nextUp, navigate])

  const toggleFullscreen = useCallback(() => {
    if (document.fullscreenElement) document.exitFullscreen()
    else containerRef.current?.requestFullscreen()
  }, [])

  // Muestra la pista elegida y apaga las demás. Los `<track>` del navegador se manejan por su modo.
  useEffect(() => {
    const textTracks = videoRef.current?.textTracks
    if (!textTracks) return
    for (let i = 0; i < textTracks.length; i++) {
      textTracks[i].mode = tracks[i]?.id === subtitle ? 'showing' : 'disabled'
    }
  }, [subtitle, tracks])

  // Los subtítulos van un poco más arriba del borde, y suben más cuando aparecen los controles
  // para no quedar tapados. Se ubican en cada línea del .vtt (en % de la altura del video,
  // midiendo hasta su borde de abajo): así funciona igual en Chrome, Firefox y la TV.
  const controlsShown = controlsVisible || !playing || menu !== null
  const placeCues = useCallback(() => {
    const textTracks = videoRef.current?.textTracks
    if (!textTracks) return
    const line = controlsShown ? CUE_LINE_WITH_CONTROLS : CUE_LINE
    for (let i = 0; i < textTracks.length; i++) {
      const track = textTracks[i]
      let moved = false
      for (const cue of track.cues ?? []) {
        if (!(cue instanceof VTTCue) || cue.line === line) continue
        cue.snapToLines = false
        cue.lineAlign = 'end'
        cue.line = line
        moved = true
      }
      // Chrome no vuelve a dibujar el subtítulo que ya está en pantalla: apagar y prender la pista lo obliga.
      if (moved && track.mode === 'showing') {
        track.mode = 'hidden'
        track.mode = 'showing'
      }
    }
  }, [controlsShown])
  useEffect(placeCues, [placeCues, subtitle])
  // Las líneas llegan cuando termina de bajar el .vtt: recién ahí se pueden ubicar. React no
  // escucha `load` en <track>, así que se engancha a mano.
  useEffect(() => {
    const elements = [...(videoRef.current?.querySelectorAll('track') ?? [])]
    for (const element of elements) element.addEventListener('load', placeCues)
    return () => {
      for (const element of elements) element.removeEventListener('load', placeCues)
    }
  }, [placeCues, tracks])

  const chooseSubtitle = useCallback(
    (id: number | null) => {
      setSubtitle(id)
      setMenu(null)
      const track = tracks.find((t) => t.id === id)
      savePreference(track?.srclang ?? 'off')
      showFlash(track ? `Subtítulos: ${track.label}` : 'Subtítulos desactivados')
      showControls()
    },
    [tracks, showFlash, showControls],
  )

  const openMenu = useCallback(() => {
    if (tracks.length === 0) return
    const current = tracks.findIndex((t) => t.id === subtitle)
    setMenu(current + 1)
    showControls()
  }, [tracks, subtitle, showControls])

  // Los botones de la barra, en orden: los de la izquierda (volver a empezar, reproducir y el
  // siguiente episodio, si hay) y los de la derecha. Con el mouse se aprietan; con el control
  // remoto se llega con Arriba.
  const controls: BarControl[] = [
    { id: 'restart', icon: RotateCcw, label: 'Desde el principio (0)', onPress: restart },
    { id: 'play', icon: playing ? Pause : Play, label: 'Reproducir o pausar (Enter)', onPress: togglePlay, filled: true },
    ...(nextUp ? [{ id: 'next', icon: SkipForward, label: 'Siguiente episodio (N)', onPress: playNext }] : []),
    { id: 'mute', icon: muted ? VolumeX : Volume2, label: muted ? 'Activar el sonido (M)' : 'Silenciar (M)', onPress: toggleMute },
    ...(tracks.length > 0
      ? [
          {
            id: 'subtitles',
            icon: Captions,
            label: 'Subtítulos (↓ o C)',
            onPress: () => (menu === null ? openMenu() : setMenu(null)),
            active: subtitle !== null,
          },
        ]
      : []),
    {
      id: 'fullscreen',
      icon: fullscreen ? Minimize : Maximize,
      label: 'Pantalla completa (F)',
      onPress: toggleFullscreen,
    },
  ]
  // Los de la izquierda llegan hasta el siguiente episodio; después va el tiempo.
  const leftCount = nextUp ? 3 : 2

  // Teclado y control remoto. Los códigos 415/19/417/412 son las teclas de reproducción
  // de los controles de TV (play, pausa, adelantar, retroceder).
  useEffect(() => {
    function onKey(event: KeyboardEvent) {
      const key = event.key
      const code = event.keyCode

      // Con el menú de subtítulos abierto, las flechas y Enter son del menú, y Atrás lo cierra
      // (sin salir del reproductor: por eso se corta la propagación).
      if (menu !== null) {
        if (key === 'ArrowUp') setMenu(Math.max(0, menu - 1))
        else if (key === 'ArrowDown') setMenu(Math.min(tracks.length, menu + 1))
        else if (key === 'Enter' || key === ' ') chooseSubtitle(menu === 0 ? null : tracks[menu - 1].id)
        else if (isBackKey(event) || key === 'c') setMenu(null)
        else return
        event.preventDefault()
        event.stopImmediatePropagation()
        showControls()
        return
      }

      // En la barra, izquierda y derecha pasan de un botón a otro, Enter lo aprieta, y Abajo o
      // Atrás salen de la barra (sin salir del reproductor).
      if (bar !== null) {
        if (key === 'ArrowLeft') setBar(Math.max(0, bar - 1))
        else if (key === 'ArrowRight') setBar(Math.min(controls.length - 1, bar + 1))
        else if (key === 'Enter' || key === ' ') {
          const control = controls[bar]
          // Los subtítulos abren su menú: la barra se suelta.
          if (control.id === 'subtitles') setBar(null)
          control.onPress()
        } else if (key === 'ArrowDown' || key === 'ArrowUp' || isBackKey(event)) setBar(null)
        else return
        event.preventDefault()
        event.stopImmediatePropagation()
        showControls()
        return
      }

      if (key === 'Enter' || key === ' ' || key === 'MediaPlayPause' || key === 'k' || code === 415 || code === 19) {
        togglePlay()
      } else if (key === 'ArrowRight' || key === 'l' || code === 417) {
        seekBy(SEEK_SECONDS)
      } else if (key === 'ArrowLeft' || key === 'j' || code === 412) {
        seekBy(-SEEK_SECONDS)
      } else if (key === 'ArrowDown' || key === 'c') {
        if (tracks.length > 0) openMenu()
        else showControls()
      } else if (key === 'ArrowUp') {
        setBar(Math.max(0, controls.findIndex((c) => c.id === 'play')))
        inBar.current = true
        showControls()
      } else if (key === '0' || key === 'Home' || key === 'MediaTrackPrevious') {
        restart()
      } else if ((key === 'n' || key === 'N' || key === 'MediaTrackNext') && nextUp) {
        playNext()
      } else if (key === 'f') {
        toggleFullscreen()
      } else if (key === 'm') {
        toggleMute()
      } else {
        return
      }
      event.preventDefault()
    }
    // En fase de captura: así este manejador corre antes que el de "Atrás" de la aplicación.
    window.addEventListener('keydown', onKey, true)
    return () => window.removeEventListener('keydown', onKey, true)
  }, [togglePlay, seekBy, showControls, toggleFullscreen, toggleMute, menu, tracks, chooseSubtitle, openMenu, bar, controls, restart, playNext, nextUp])

  useEffect(() => {
    const onChange = () => setFullscreen(document.fullscreenElement !== null)
    document.addEventListener('fullscreenchange', onChange)
    return () => document.removeEventListener('fullscreenchange', onChange)
  }, [])

  // Retomar donde quedó, salvo que ya lo haya terminado o esté al principio o al final.
  function onLoaded(video: HTMLVideoElement) {
    setDuration(video.duration)
    const progress = detail.progress
    if (progress && !progress.watched && progress.position > 10 && progress.position < video.duration - 30) {
      video.currentTime = progress.position
      showFlash(`Continuando desde ${formatDuration(progress.position)}`, 3500)
    }
  }

  function seekTo(event: MouseEvent<HTMLDivElement>) {
    const video = videoRef.current
    if (!video || !duration) return
    const bar = event.currentTarget.getBoundingClientRect()
    video.currentTime = ((event.clientX - bar.left) / bar.width) * duration
    showControls()
  }

  const share = duration ? time / duration : 0

  return (
    <div
      ref={containerRef}
      className={`player ${controlsShown ? 'show-controls' : ''}`}
      onMouseMove={showControls}
    >
      <video
        ref={videoRef}
        src={mediaUrl(`/api/files/${file.id}/stream`)}
        autoPlay
        onClick={togglePlay}
        onLoadedMetadata={(e) => onLoaded(e.currentTarget)}
        onDurationChange={(e) => setDuration(e.currentTarget.duration)}
        onVolumeChange={(e) => setMuted(e.currentTarget.muted)}
        // Cargando: al empezar, al adelantar o atrasar, o si la red no alcanza.
        onWaiting={() => setBuffering(true)}
        onSeeking={() => setBuffering(true)}
        onCanPlay={() => setBuffering(false)}
        onPlaying={() => setBuffering(false)}
        onSeeked={(e) => {
          if (e.currentTarget.readyState >= 3) setBuffering(false)
          report(e.currentTarget.paused ? 'paused' : 'playing')
        }}
        onTimeUpdate={(e) => {
          const video = e.currentTarget
          setTime(video.currentTime)
          // Se guarda siempre la última posición: el aviso final al salir la usa.
          last.current = {
            position: video.currentTime,
            duration: Number.isFinite(video.duration) ? video.duration : null,
          }
        }}
        onPlay={() => {
          setPlaying(true)
          report('playing')
          showControls()
        }}
        onPause={() => {
          setPlaying(false)
          report('paused')
          setControlsVisible(true)
        }}
      >
        {tracks.map((track) => (
          <track
            key={track.id}
            kind="subtitles"
            src={mediaUrl(`/api/subtitles/${track.id}`)}
            srcLang={track.srclang ?? undefined}
            label={track.label}
          />
        ))}
      </video>

      <div className="player-top">
        <button className="player-icon" onClick={() => navigate(-1)} title="Volver (Esc)" tabIndex={-1}>
          <ArrowLeft />
        </button>
        <span className="player-title">{title}</span>
      </div>

      {flash && <div className="player-flash">{flash}</div>}
      {showSpinner ? (
        <div className="player-center" role="status" aria-label="Cargando">
          <LoaderCircle className="spin" />
        </div>
      ) : (
        !playing && (
          <div className="player-center" aria-hidden="true">
            <Play fill="currentColor" className="play-icon" />
          </div>
        )
      )}

      {menu !== null && (
        <div className="subtitle-menu" role="listbox" aria-label="Subtítulos">
          <div className="subtitle-menu-title">Subtítulos</div>
          {[null, ...tracks].map((track, i) => {
            const active = (track?.id ?? null) === subtitle
            return (
              <button
                key={track?.id ?? 'off'}
                role="option"
                aria-selected={active}
                className={`subtitle-option ${i === menu ? 'highlighted' : ''}`}
                onClick={() => chooseSubtitle(track?.id ?? null)}
                onMouseEnter={() => setMenu(i)}
                tabIndex={-1}
              >
                <span className="check">{active && <Check />}</span>
                {track?.label ?? 'Desactivados'}
              </button>
            )
          })}
        </div>
      )}

      <div className="player-bottom">
        <div className="player-progress" onClick={seekTo}>
          <div className="player-progress-fill" style={{ width: `${share * 100}%` }} />
        </div>
        {bar !== null && <div className="player-hint">{controls[bar].label}</div>}
        <div className="player-bar">
          {controls.slice(0, leftCount).map((control, i) => (
            <BarButton key={control.id} control={control} highlighted={bar === i} />
          ))}
          <span className="player-time">
            {formatDuration(time)} / {formatDuration(duration || null)}
          </span>
          {controls.slice(leftCount).map((control, i) => (
            <BarButton key={control.id} control={control} highlighted={bar === leftCount + i} />
          ))}
        </div>
      </div>
    </div>
  )
}

function BarButton({ control, highlighted }: { control: BarControl; highlighted: boolean }) {
  const Icon = control.icon
  return (
    <button
      className={`player-icon ${control.active ? 'active' : ''} ${highlighted ? 'is-focused' : ''}`}
      onClick={control.onPress}
      title={control.label}
      aria-label={control.label}
      tabIndex={-1}
    >
      <Icon fill={control.filled ? 'currentColor' : 'none'} />
    </button>
  )
}
