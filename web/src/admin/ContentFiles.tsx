// Los archivos de una película o episodio, con sus versiones y subtítulos: quitar archivos,
// agregar, renombrar y quitar subtítulos.

import { useState, type FormEvent } from 'react'
import { AudioLines, Captions, Check, FilePlay, Film, Plus, Search, Trash, X } from 'lucide-react'
import { api } from '../shared/api'
import { formatDuration, formatSize } from '../shared/format'
import { isSubtitle } from '../shared/upload'
import { resolution } from '../shared/versions'
import { languageName } from '../shared/languages'
import type { ContentDetail } from '../shared/types/ContentDetail'
import type { MediaFile } from '../shared/types/MediaFile'
import type { SubtitleTrack } from '../shared/types/SubtitleTrack'
import { Alert } from './ui'
import { SubtitleSearch } from './SubtitleSearch'

export function Files({ node, onChange }: { node: ContentDetail; onChange: () => void }) {
  if (node.files.length === 0) return <p className="tree-note muted">Sin archivo todavía.</p>
  return (
    <>
      {node.files.map((file) => (
        <FileRow
          key={file.id}
          file={file}
          subtitles={node.subtitles.filter((s) => s.media_file_id === file.id)}
          onChange={onChange}
        />
      ))}
    </>
  )
}

function FileRow({
  file,
  subtitles,
  onChange,
}: {
  file: MediaFile
  subtitles: SubtitleTrack[]
  onChange: () => void
}) {
  const [adding, setAdding] = useState(false)
  const [managing, setManaging] = useState(false)
  const [searching, setSearching] = useState(false)
  const [error, setError] = useState<string>()
  const name = file.rel_path.split('/').pop()
  const labels = subtitles.map((s) => s.label).join(', ')

  async function removeFile() {
    if (!confirm(`¿Quitar ${name}? El video y sus subtítulos van a la papelera de la biblioteca.`)) return
    setError(undefined)
    try {
      await api(`/api/admin/files/${file.id}`, { method: 'DELETE' })
      onChange()
    } catch (err) {
      setError((err as Error).message)
    }
  }

  async function remove(track: SubtitleTrack) {
    if (!confirm(`¿Quitar los subtítulos "${track.label}"? Se borran del disco.`)) return
    setError(undefined)
    try {
      await api(`/api/admin/subtitles/${track.id}`, { method: 'DELETE' })
      onChange()
    } catch (err) {
      setError((err as Error).message)
    }
  }

  return (
    <div className="file-row">
      {/* Nombre y datos técnicos juntos, como un solo elemento; los subtítulos van aparte, en una
          línea con sus nombres. Los chips para renombrar y quitar aparecen al tocar "Editar". */}
      <div className="file-row-head">
        <div className="file-row-main">
          <FilePlay className="icon" />
          <span className="mono grow truncate" title={file.rel_path}>
            {name}
          </span>
          <span className="small muted">
            {formatDuration(file.duration)} · {formatSize(file.size)}
          </span>
          <button
            className="btn btn-ghost btn-sm btn-icon btn-danger"
            title="Quitar este archivo: el video y sus subtítulos van a la papelera de la biblioteca"
            aria-label={`Quitar ${name}`}
            onClick={removeFile}
          >
            <Trash />
          </button>
        </div>
        {file.tech && <TechLine file={file} />}
      </div>
      {!managing && !adding ? (
        <div className="file-subtitles-summary small">
          <Captions className="icon" />
          {subtitles.length > 0 ? (
            <>
              <span className="truncate" title={labels}>
                {labels}
              </span>
              <button className="link-button" onClick={() => setManaging(true)}>
                Editar
              </button>
            </>
          ) : (
            <button className="link-button" onClick={() => setAdding(true)}>
              Agregar subtítulo
            </button>
          )}
          {!searching && (
            <button className="link-button" onClick={() => setSearching(true)}>
              Buscar
            </button>
          )}
        </div>
      ) : (
        <div className="file-subtitles">
          {subtitles.map((track) => (
            <SubtitleChip
              key={track.id}
              track={track}
              onRemove={() => remove(track)}
              onRename={async (label) => {
                setError(undefined)
                try {
                  await api(`/api/admin/subtitles/${track.id}`, { method: 'PATCH', body: { label } })
                  onChange()
                  return true
                } catch (err) {
                  setError((err as Error).message)
                  return false
                }
              }}
            />
          ))}
          {!adding && (
            <>
              <button className="btn btn-ghost btn-sm" onClick={() => setAdding(true)}>
                <Plus /> Agregar subtítulo
              </button>
              {!searching && (
                <button className="btn btn-ghost btn-sm" onClick={() => setSearching(true)}>
                  <Search /> Buscar en OpenSubtitles
                </button>
              )}
              <button className="btn btn-ghost btn-sm" onClick={() => setManaging(false)}>
                Listo
              </button>
            </>
          )}
        </div>
      )}
      {error && <Alert>{error}</Alert>}
      {searching && <SubtitleSearch fileId={file.id} onAdded={onChange} onClose={() => setSearching(false)} />}
      {adding && (
        <AddSubtitleForm
          fileId={file.id}
          onDone={() => {
            setAdding(false)
            onChange()
          }}
          onCancel={() => setAdding(false)}
        />
      )}
    </div>
  )
}

/**
 * Lo técnico de un archivo en una línea: imagen ("1080p · HEVC Main 10 · 10 bits · SDR ·
 * 23,976 fps · 2,1 Mbps") y cada pista de audio ("AAC 2.0 japonés").
 */
function TechLine({ file }: { file: MediaFile }) {
  const tech = file.tech!
  const video = [
    resolution(file),
    [tech.video_codec, tech.video_profile].filter(Boolean).join(' '),
    tech.bit_depth && `${tech.bit_depth} bits`,
    tech.dynamic_range,
    tech.frame_rate && `${formatNumber(tech.frame_rate, 3)} fps`,
    tech.bitrate && `${formatNumber(tech.bitrate / 1_000_000, 1)} Mbps`,
  ].filter(Boolean)
  return (
    <div className="tech-line small">
      <span className="tech-group" title="Video">
        <Film className="icon" />
        {video.map((part, i) => (
          <span key={i} className={part === tech.dynamic_range && part !== 'SDR' ? 'tech-hdr' : undefined}>
            {part}
          </span>
        ))}
      </span>
      {tech.audio.map((audio, i) => (
        <span key={i} className="tech-group" title={`Audio ${i + 1}`}>
          <AudioLines className="icon" />
          <span>
            {audio.codec} {audio.channels}
          </span>
          {languageName(audio.language) && <span>{languageName(audio.language)}</span>}
        </span>
      ))}
    </div>
  )
}

/** Con coma decimal y sin ceros de más: 23,976 · 25 · 2,1. */
function formatNumber(value: number, decimals: number) {
  return value.toLocaleString('es-AR', { maximumFractionDigits: decimals })
}

/** Un subtítulo del archivo. Tocar el nombre lo deja editar; al pasar el mouse dice de dónde salió. */
function SubtitleChip({
  track,
  onRemove,
  onRename,
}: {
  track: SubtitleTrack
  onRemove: () => void
  onRename: (label: string) => Promise<boolean>
}) {
  const [editing, setEditing] = useState(false)
  const [label, setLabel] = useState(track.label)
  const origin = track.source_name ? `Archivo: ${track.source_name}` : 'Venía dentro del video'

  async function save(event: FormEvent) {
    event.preventDefault()
    if (label.trim() === track.label || (await onRename(label))) setEditing(false)
  }

  if (editing) {
    return (
      <form className="sub-chip-edit" onSubmit={save}>
        <input
          className="input"
          value={label}
          onChange={(e) => setLabel(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Escape') {
              setLabel(track.label)
              setEditing(false)
            }
          }}
          aria-label="Nombre del subtítulo"
          autoFocus
        />
        <button className="btn btn-primary btn-sm btn-icon" type="submit" title="Guardar">
          <Check />
        </button>
      </form>
    )
  }

  return (
    <span className="badge sub-chip" title={origin}>
      <Captions />
      <button className="sub-chip-label" onClick={() => setEditing(true)} title={`${origin} · tocá para renombrar`}>
        {track.label}
      </button>
      <button className="sub-chip-remove" title="Quitar" onClick={onRemove}>
        <X />
      </button>
    </span>
  )
}

const LANGUAGES: [code: string, name: string][] = [
  ['spa', 'Español'],
  ['eng', 'Inglés'],
  ['por', 'Portugués'],
  ['jpn', 'Japonés'],
  ['fra', 'Francés'],
  ['deu', 'Alemán'],
  ['ita', 'Italiano'],
  ['und', 'Sin idioma'],
]

// Palabras que suelen aparecer en el nombre de un subtítulo ("Frieren 01.es.srt", "Spanish.srt").
const LANGUAGE_HINTS: Record<string, string> = {
  es: 'spa', spa: 'spa', esp: 'spa', spanish: 'spa', espanol: 'spa', español: 'spa', lat: 'spa', latino: 'spa',
  en: 'eng', eng: 'eng', english: 'eng',
  pt: 'por', por: 'por', pob: 'por', ptbr: 'por', portuguese: 'por',
  ja: 'jpn', jpn: 'jpn', jp: 'jpn', japanese: 'jpn',
  fr: 'fra', fre: 'fra', fra: 'fra', french: 'fra',
  de: 'deu', ger: 'deu', deu: 'deu', german: 'deu',
  it: 'ita', ita: 'ita', italian: 'ita',
}

// Variantes regionales que suelen aparecer en el nombre del archivo.
const REGIONS: [pattern: RegExp, language: string, region: string][] = [
  [/latin|latam|latino|\bes[-_]?(419|mx|ar)\b/i, 'spa', 'Latinoamérica'],
  [/castellano|spain|españa|\bes[-_]?es\b/i, 'spa', 'España'],
  [/brazil|brasil|\bpt[-_]?br\b|\bpob\b/i, 'por', 'Brasil'],
  [/portugal|\bpt[-_]?pt\b/i, 'por', 'Portugal'],
]

/**
 * El nombre que se propone: el idioma, la región si el archivo la dice ("Spanish (Latin America)"
 * → "Español (Latinoamérica)") y si son forzados.
 */
function suggestLabel(fileName: string, language: string, forced: boolean) {
  let label = LANGUAGES.find(([code]) => code === language)?.[1] ?? 'Subtítulos'
  if (language === 'und') label = 'Subtítulos'
  const region = REGIONS.find(([pattern, lang]) => lang === language && pattern.test(fileName))
  if (region) label += ` (${region[2]})`
  if (forced) label += ' (forzados)'
  return label
}

/** El idioma que sugiere el nombre del archivo, mirando desde el final. Si no dice nada, español. */
function guessLanguage(fileName: string) {
  const words = fileName.toLowerCase().split(/[^\p{L}]+/u).reverse()
  return words.map((w) => LANGUAGE_HINTS[w]).find(Boolean) ?? 'spa'
}

function AddSubtitleForm({ fileId, onDone, onCancel }: { fileId: number; onDone: () => void; onCancel: () => void }) {
  const [file, setFile] = useState<File>()
  const [language, setLanguage] = useState('spa')
  const [forced, setForced] = useState(false)
  // El nombre se propone solo hasta que se lo escribe a mano.
  const [label, setLabel] = useState('')
  const [labelEdited, setLabelEdited] = useState(false)
  const [sending, setSending] = useState(false)
  const [error, setError] = useState<string>()
  const shownLabel = labelEdited ? label : suggestLabel(file?.name ?? '', language, forced)

  function choose(chosen: File | undefined) {
    setError(undefined)
    if (chosen && !isSubtitle(chosen.name)) {
      setError(`${chosen.name} no es un subtítulo: se aceptan SRT, ASS, SSA y VTT.`)
      setFile(undefined)
      return
    }
    setFile(chosen)
    if (chosen) {
      setLanguage(guessLanguage(chosen.name))
      setForced(/forced|forzad/i.test(chosen.name))
    }
  }

  async function submit(event: FormEvent) {
    event.preventDefault()
    if (!file) return
    setSending(true)
    setError(undefined)
    const params = new URLSearchParams({ name: file.name, language, forced: String(forced), label: shownLabel })
    try {
      await api(`/api/admin/files/${fileId}/subtitles?${params}`, { method: 'POST', file })
      onDone()
    } catch (err) {
      setError((err as Error).message)
      setSending(false)
    }
  }

  return (
    <form className="subtitle-form" onSubmit={submit}>
      <div className="inline-form">
        <input
          className="input"
          type="file"
          accept=".srt,.ass,.ssa,.vtt"
          onChange={(e) => choose(e.target.files?.[0])}
          required
          autoFocus
        />
        <select
          className="select"
          value={language}
          onChange={(e) => setLanguage(e.target.value)}
          aria-label="Idioma"
        >
          {LANGUAGES.map(([code, name]) => (
            <option key={code} value={code}>
              {name}
            </option>
          ))}
        </select>
        <input
          className="input"
          value={shownLabel}
          onChange={(e) => {
            setLabel(e.target.value)
            setLabelEdited(true)
          }}
          placeholder="Nombre"
          aria-label="Nombre en el menú del Player"
          title="Cómo aparece en el menú de subtítulos del Player"
          required
        />
        <label className="row small" title="Solo traducen las partes en otro idioma (carteles, diálogos sueltos)">
          <input type="checkbox" checked={forced} onChange={(e) => setForced(e.target.checked)} />
          Forzados
        </label>
        <button className="btn btn-primary btn-sm" type="submit" disabled={!file || sending}>
          {sending ? 'Agregando…' : 'Agregar'}
        </button>
        <button className="btn btn-ghost btn-sm" type="button" onClick={onCancel} disabled={sending}>
          Cancelar
        </button>
      </div>
      {error && <Alert>{error}</Alert>}
    </form>
  )
}
