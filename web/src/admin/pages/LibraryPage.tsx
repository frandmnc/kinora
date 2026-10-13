import { useState, type FormEvent } from 'react'
import { Link, useParams } from 'react-router'
import { ArrowLeft, Captions, Check, ChevronDown, ChevronRight, Clapperboard, FilePlay, Plus, Trash, Tv, X } from 'lucide-react'
import { api } from '../../shared/api'
import { useApi } from '../../shared/useApi'
import { formatDuration, formatSize } from '../../shared/format'
import { isSubtitle } from '../../shared/upload'
import { resolution } from '../../shared/versions'
import type { Content } from '../../shared/types/Content'
import type { ContentDetail } from '../../shared/types/ContentDetail'
import type { Library } from '../../shared/types/Library'
import type { MediaFile } from '../../shared/types/MediaFile'
import type { SubtitleTrack } from '../../shared/types/SubtitleTrack'
import { Alert, Card, EmptyState, PageHeader } from '../ui'

/** Una biblioteca por dentro: sus películas y series, con sus archivos y subtítulos. */
export function LibraryPage() {
  const id = Number(useParams().id)
  const { data: libraries } = useApi<Library[]>('/api/admin/libraries')
  const { data: contents, error, loading } = useApi<Content[]>(`/api/contents?library_id=${id}`)
  const library = libraries?.find((l) => l.id === id)

  return (
    <>
      <Link to="/libraries" className="btn btn-ghost btn-sm back-link">
        <ArrowLeft /> Bibliotecas
      </Link>
      <PageHeader
        title={library?.name ?? 'Biblioteca'}
        description={library && <span className="mono">{library.dir}/</span>}
      />

      {error && <Alert>No se pudo cargar el contenido: {error}</Alert>}

      <Card
        title="Contenido"
        description={contents && contents.length > 0 ? `${contents.length} título(s)` : undefined}
      >
        {loading && !contents && <p className="card-body muted">Cargando…</p>}
        {contents?.length === 0 && (
          <EmptyState icon={Clapperboard} title="Esta biblioteca está vacía">
            Lo que importes desde <Link to="/import">Importar</Link> aparece acá.
          </EmptyState>
        )}
        {contents?.map((content) => <ContentRow key={content.id} content={content} />)}
      </Card>
    </>
  )
}

/** Una película o serie. Al abrirla se piden sus temporadas, episodios y archivos. */
function ContentRow({ content }: { content: Content }) {
  const [open, setOpen] = useState(false)
  const Icon = content.kind === 'show' ? Tv : Clapperboard

  return (
    <div className="tree-item">
      <button className="list-row tree-toggle" onClick={() => setOpen(!open)} aria-expanded={open}>
        {open ? <ChevronDown className="icon" /> : <ChevronRight className="icon" />}
        <Icon className="icon" />
        <strong className="grow truncate">{content.title}</strong>
        <span className="small muted">
          {content.kind === 'show' ? 'Serie' : 'Película'}
          {content.year && ` · ${content.year}`}
        </span>
      </button>
      {open && <ContentTree id={content.id} />}
    </div>
  )
}

function ContentTree({ id }: { id: number }) {
  const { data: detail, error, reload } = useApi<ContentDetail>(`/api/contents/${id}`)

  if (error) {
    return (
      <div className="card-body">
        <Alert>{error}</Alert>
      </div>
    )
  }
  if (!detail) return <p className="card-body muted">Cargando…</p>

  // Película: sus archivos directamente. Serie: temporada → episodio → archivos.
  if (detail.content.kind === 'movie') {
    return (
      <div className="tree-children">
        <Files node={detail} onChange={reload} />
      </div>
    )
  }
  return (
    <div className="tree-children">
      {detail.children.length === 0 && <p className="tree-note muted">No tiene temporadas.</p>}
      {detail.children.map((season) => (
        <div key={season.content.id} className="tree-season">
          <div className="tree-heading">{season.content.title}</div>
          {season.children.map((episode) => (
            <div key={episode.content.id} className="tree-episode">
              <div className="tree-episode-title">
                <span className="muted">E{String(episode.content.number ?? 0).padStart(2, '0')}</span>{' '}
                {episode.content.title}
              </div>
              <Files node={episode} onChange={reload} />
            </div>
          ))}
        </div>
      ))}
    </div>
  )
}

function Files({ node, onChange }: { node: ContentDetail; onChange: () => void }) {
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
  const [error, setError] = useState<string>()
  const name = file.rel_path.split('/').pop()

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
      <div className="file-row-main">
        <FilePlay className="icon" />
        <span className="mono grow truncate" title={file.rel_path}>
          {name}
        </span>
        <span className="small muted">
          {resolution(file)} · {formatDuration(file.duration)} · {formatSize(file.size)}
        </span>
        <button
          className="btn btn-ghost btn-sm btn-danger"
          title="Saca el archivo del catálogo; el video y sus subtítulos van a la papelera de la biblioteca"
          onClick={removeFile}
        >
          <Trash /> Quitar
        </button>
      </div>
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
          <button className="btn btn-ghost btn-sm" onClick={() => setAdding(true)}>
            <Plus /> Agregar subtítulo
          </button>
        )}
      </div>
      {error && <Alert>{error}</Alert>}
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
