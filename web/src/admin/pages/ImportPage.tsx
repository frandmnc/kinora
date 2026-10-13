import { Fragment, useEffect, useState, type DragEvent, type FormEvent } from 'react'
import { Link } from 'react-router'
import {
  ArrowLeft,
  Captions,
  Check,
  CircleCheck,
  CircleX,
  Clock,
  CloudUpload,
  FilePlay,
  Inbox as InboxIcon,
  LoaderCircle,
  Repeat,
  SkipForward,
  Trash,
  TriangleAlert,
} from 'lucide-react'
import { api } from '../../shared/api'
import { useApi } from '../../shared/useApi'
import { formatDuration, formatSize } from '../../shared/format'
import { isVideo, uploadFile, videosFromDrop } from '../../shared/upload'
import type { InboxFile } from '../../shared/types/InboxFile'
import type { MediaDirSetting } from '../../shared/types/MediaDirSetting'
import type { Library } from '../../shared/types/Library'
import type { FileAnalysis } from '../../shared/types/FileAnalysis'
import type { ImportRequest } from '../../shared/types/ImportRequest'
import type { ImportResult } from '../../shared/types/ImportResult'
import type { ImportTarget } from '../../shared/types/ImportTarget'
import type { ImportAction } from '../../shared/types/ImportAction'
import type { ConflictChoice } from '../../shared/types/ConflictChoice'
import type { Content } from '../../shared/types/Content'
import type { ContentDetail } from '../../shared/types/ContentDetail'
import { Alert, Badge, Card, EmptyState, Meter, OkBadge, PageHeader } from '../ui'
import { OPERATION_LABELS } from '../labels'

/**
 * Importar en tres pasos: subir archivos a la bandeja → revisar el análisis y elegir el destino → ver el resultado.
 * El destino es un contenido nuevo o uno existente; si un episodio ya tiene archivo, se elige qué hacer.
 * Los que necesitan remux pasan por la cola; convertir el video llega en la Etapa 4.
 */
export function ImportPage() {
  const [analysis, setAnalysis] = useState<FileAnalysis[]>()
  const [result, setResult] = useState<ImportResult>()
  const [busy, setBusy] = useState<string>()
  const [error, setError] = useState<string>()

  async function run<T>(message: string, action: () => Promise<T>): Promise<T | undefined> {
    setBusy(message)
    setError(undefined)
    try {
      return await action()
    } catch (err) {
      setError((err as Error).message)
    } finally {
      setBusy(undefined)
    }
  }

  async function analyze(names: string[]) {
    const files = await run('Analizando con ffprobe…', () =>
      api<FileAnalysis[]>('/api/admin/imports/analyze', { method: 'POST', body: { files: names } }),
    )
    if (files) setAnalysis(files)
  }

  async function importFiles(request: ImportRequest) {
    const imported = await run('Importando…', () =>
      api<ImportResult>('/api/admin/imports', { method: 'POST', body: request }),
    )
    if (imported) setResult(imported)
  }

  function restart() {
    setAnalysis(undefined)
    setResult(undefined)
    setError(undefined)
  }

  const step = result ? 3 : analysis ? 2 : 1
  const ready = analysis?.filter(isReady) ?? []
  const notReady = (analysis?.length ?? 0) - ready.length

  return (
    <>
      <PageHeader
        title="Importar"
        description="Los archivos en MP4 con HEVC y AAC se mueven tal cual. Los MKV con video HEVC pasan por la cola para cambiarles la caja (remux), que tarda segundos. Convertir otros videos llega en la Etapa 4."
        actions={<Stepper current={step} />}
      />

      {busy && (
        <Alert tone="info">
          <span className="row">
            <LoaderCircle className="spin" size={16} /> {busy}
          </span>
        </Alert>
      )}
      {error && <Alert>{error}</Alert>}

      {result ? (
        <ResultView result={result} onRestart={restart} />
      ) : analysis ? (
        <>
          <AnalysisTable files={analysis} />
          {/* Los que no se pueden importar todavía se dejan de lado: siguen en la bandeja. */}
          {ready.length > 0 && ready.length < analysis.length && (
            <Alert tone="warning">
              {notReady} archivo{notReady === 1 ? '' : 's'} no se puede{notReady === 1 ? '' : 'n'} importar
              todavía: se deja{notReady === 1 ? '' : 'n'} de lado y sigue{notReady === 1 ? '' : 'n'} en la bandeja.
              Se importa{ready.length === 1 ? '' : 'n'} {ready.length === 1 ? 'el otro' : `los otros ${ready.length}`}.
            </Alert>
          )}
          {ready.length > 0 ? (
            <DestinationForm files={ready} disabled={!!busy} onImport={importFiles} onBack={restart} />
          ) : (
            <Alert tone="warning">
              Ninguno de estos archivos se puede importar todavía.{' '}
              <button className="btn btn-sm" onClick={restart}>
                <ArrowLeft /> Elegir otros
              </button>
            </Alert>
          )}
        </>
      ) : (
        <Inbox disabled={!!busy} onAnalyze={analyze} />
      )}
    </>
  )
}

function Stepper({ current }: { current: number }) {
  const steps = ['Subir', 'Revisar y elegir destino', 'Listo']
  return (
    <div className="stepper">
      {steps.map((label, i) => {
        const number = i + 1
        const state = number < current ? 'done' : number === current ? 'current' : ''
        return (
          <Fragment key={label}>
            {i > 0 && <span className="step-separator" />}
            <span className={`step ${state}`}>
              <span className="step-number">{number < current ? <Check /> : number}</span>
              {label}
            </span>
          </Fragment>
        )
      })}
    </div>
  )
}

function isReady(file: FileAnalysis) {
  return !file.error && file.blockers.length === 0
}

// --- Paso 1: subir archivos a la bandeja de entrada ---

type Upload = { name: string; size: number; sent: number; error?: string }

function Inbox({ disabled, onAnalyze }: { disabled: boolean; onAnalyze: (names: string[]) => void }) {
  const { data: mediaDir } = useApi<MediaDirSetting>('/api/admin/settings/media-dir')
  const { data: inbox, error, reload } = useApi<InboxFile[]>('/api/admin/uploads')
  const [uploads, setUploads] = useState<Upload[]>([])
  const [selected, setSelected] = useState<Set<string>>(new Set())
  const [dragging, setDragging] = useState(false)
  const uploading = uploads.some((u) => u.sent < u.size && !u.error)

  // Al cargar la bandeja, se marca todo para analizar.
  useEffect(() => {
    if (inbox) setSelected(new Set(inbox.map((file) => file.name)))
  }, [inbox])

  // Sube los archivos de a uno, mostrando el avance de cada uno.
  async function upload(files: File[]) {
    const queue: Upload[] = files.map((file) => ({ name: file.name, size: file.size, sent: 0 }))
    setUploads(queue)
    const update = (i: number, change: Partial<Upload>) =>
      setUploads((current) => current.map((u, j) => (j === i ? { ...u, ...change } : u)))

    for (const [i, file] of files.entries()) {
      try {
        await uploadFile(file, (sent) => update(i, { sent }))
        update(i, { sent: file.size })
      } catch (err) {
        update(i, { error: (err as Error).message })
      }
    }
    reload()
  }

  async function onDrop(event: DragEvent) {
    event.preventDefault()
    setDragging(false)
    const files = await videosFromDrop(event.dataTransfer)
    if (files.length > 0) upload(files)
  }

  async function discard(name: string) {
    await api(`/api/admin/uploads/${encodeURIComponent(name)}`, { method: 'DELETE' })
    reload()
  }

  function toggle(name: string) {
    const next = new Set(selected)
    if (next.has(name)) next.delete(name)
    else next.add(name)
    setSelected(next)
  }

  if (mediaDir && !mediaDir.path) {
    return (
      <Alert tone="warning">
        Primero elegí la carpeta media en <Link to="/settings">Configuración</Link>.
      </Alert>
    )
  }

  return (
    <>
      <Card title="Subir archivos" description="se guardan en la bandeja de entrada, dentro de la carpeta media">
        <label
          className={`dropzone ${dragging ? 'dragging' : ''}`}
          onDragOver={(e) => {
            e.preventDefault()
            setDragging(true)
          }}
          onDragLeave={() => setDragging(false)}
          onDrop={onDrop}
        >
          <CloudUpload />
          <strong>Soltá acá videos o una carpeta</strong>
          <span>o hacé clic para elegirlos · MKV, MP4, MOV, AVI…</span>
          <input
            type="file"
            multiple
            hidden
            disabled={uploading}
            onChange={(e) => {
              const files = [...(e.target.files ?? [])].filter((f) => isVideo(f.name))
              e.target.value = ''
              if (files.length > 0) upload(files)
            }}
          />
        </label>
        {uploads.map((u) => (
          <div key={u.name} className="list-row">
            <FilePlay className="icon" />
            <span className="mono grow truncate">{u.name}</span>
            {u.error ? (
              <Badge tone="critical" icon={CircleX}>
                {u.error}
              </Badge>
            ) : u.sent >= u.size ? (
              <OkBadge>Subido</OkBadge>
            ) : (
              <>
                <Meter value={u.sent / u.size} small label={`Subiendo ${u.name}`} />
                <span className="small muted" style={{ width: 150, textAlign: 'right' }}>
                  {formatSize(u.sent)} / {formatSize(u.size)}
                </span>
              </>
            )}
          </div>
        ))}
      </Card>

      <Card
        title="Bandeja de entrada"
        description={inbox && inbox.length > 0 ? `${inbox.length} archivo(s) esperando` : undefined}
        actions={
          <button
            className="btn btn-primary"
            disabled={disabled || uploading || selected.size === 0}
            onClick={() => onAnalyze([...selected])}
          >
            Analizar {selected.size > 0 && `(${selected.size})`}
          </button>
        }
      >
        {error && (
          <div className="card-body">
            <Alert>{error}</Alert>
          </div>
        )}
        {inbox?.length === 0 && (
          <EmptyState icon={InboxIcon} title="La bandeja está vacía">
            Lo que subas aparece acá hasta que lo importes o lo descartes.
          </EmptyState>
        )}
        {inbox?.map((file) => (
          <label key={file.name} className="list-row">
            <input type="checkbox" checked={selected.has(file.name)} onChange={() => toggle(file.name)} />
            <FilePlay className="icon" />
            <span className="mono grow truncate">{file.name}</span>
            <span className="small muted">{formatSize(file.size)}</span>
            <button
              className="btn btn-ghost btn-sm btn-danger"
              onClick={(e) => {
                e.preventDefault()
                if (confirm(`¿Descartar ${file.name}? Se borra de la bandeja.`)) discard(file.name)
              }}
            >
              <Trash /> Descartar
            </button>
          </label>
        ))}
      </Card>
    </>
  )
}

// --- Paso 2: análisis y destino ---

function AnalysisTable({ files }: { files: FileAnalysis[] }) {
  return (
    <Card title="Análisis" description="lo que encontró ffprobe en cada archivo">
      <div className="table-scroll">
        <table className="table">
          <thead>
            <tr>
              <th>Archivo</th>
              <th>Duración</th>
              <th>Video</th>
              <th>Audio</th>
              <th>Subtítulos</th>
              <th>Estado</th>
            </tr>
          </thead>
          <tbody>
            {files.map((file) => (
              <tr key={file.name}>
                <td>
                  <div className="mono">{file.name}</div>
                  <div className="small muted">{formatSize(file.size)}</div>
                </td>
                <td className="secondary">{formatDuration(file.duration)}</td>
                <td className="secondary">{file.video ?? '—'}</td>
                <td className="secondary">{file.audio.join(', ') || '—'}</td>
                <td className="secondary">{file.subtitles.join(', ') || '—'}</td>
                <td>
                  {file.error ? (
                    <Badge tone="critical" icon={CircleX}>
                      No se pudo leer
                    </Badge>
                  ) : file.blockers.length > 0 ? (
                    <ul className="problems">
                      {file.blockers.map((blocker) => (
                        <li key={blocker}>
                          <Badge tone="warning" icon={CircleX}>
                            {blocker}
                          </Badge>
                        </li>
                      ))}
                    </ul>
                  ) : file.operation === 'copy' ? (
                    <OkBadge>Listo: se mueve tal cual</OkBadge>
                  ) : (
                    <div className="operation">
                      <Badge tone="accent" icon={Repeat}>
                        {OPERATION_LABELS[file.operation ?? 'remux']}
                      </Badge>
                      <span className="small muted">{file.changes.join(' · ')}</span>
                    </div>
                  )}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </Card>
  )
}

type Mode = 'new' | 'existing'
type Choices = Record<string, ConflictChoice | ''>

const CHOICE_LABELS: Record<ConflictChoice, string> = {
  replace: 'Reemplazar',
  add_version: 'Agregar como otra versión',
  skip: 'Omitir',
}

/** Para comparar títulos: "Sousou no Frieren" y "sousou-no frieren" son lo mismo. */
function normalize(title: string) {
  return title.toLowerCase().replace(/[^\p{L}\p{N}]+/gu, '')
}

function DestinationForm({
  files,
  disabled,
  onImport,
  onBack,
}: {
  files: FileAnalysis[]
  disabled: boolean
  onImport: (request: ImportRequest) => void
  onBack: () => void
}) {
  const { data: libraries } = useApi<Library[]>('/api/admin/libraries')
  const first = files[0].guess
  const canBeMovie = files.length === 1

  const [libraryId, setLibraryId] = useState<number>()
  const selectedLibrary = libraryId ?? libraries?.[0]?.id
  const { data: contents } = useApi<Content[]>(selectedLibrary ? `/api/contents?library_id=${selectedLibrary}` : '')
  // Si hay varios archivos, solo pueden ir a una serie.
  const candidates = contents?.filter((c) => c.kind === 'show' || canBeMovie) ?? []

  const [mode, setMode] = useState<Mode>('new')
  const [existingId, setExistingId] = useState<number>()
  const [kind, setKind] = useState<'movie' | 'show'>(canBeMovie && first.episode === null ? 'movie' : 'show')
  const [title, setTitle] = useState(first.title ?? '')
  const [year, setYear] = useState(first.year?.toString() ?? '')
  const [season, setSeason] = useState(String(first.season ?? 1))
  const [episodes, setEpisodes] = useState<Record<string, string>>(() =>
    Object.fromEntries(files.map((file, i) => [file.name, String(file.guess.episode ?? i + 1)])),
  )
  const [choices, setChoices] = useState<Choices>({})

  // Si el nombre del archivo coincide con algo que ya está en la biblioteca, se propone agregarlo ahí.
  useEffect(() => {
    if (!contents) return
    const match = first.title
      ? contents.find((c) => (c.kind === 'show' || canBeMovie) && normalize(c.title) === normalize(first.title!))
      : undefined
    setMode(match ? 'existing' : 'new')
    setExistingId(match?.id)
  }, [contents, first.title, canBeMovie])

  const existing = candidates.find((c) => c.id === existingId)
  const { data: detail } = useApi<ContentDetail>(existing ? `/api/contents/${existing.id}` : '')
  const tree = existing && detail?.content.id === existing.id ? detail : undefined

  // Conflictos: archivos cuyo destino ya tiene archivo. Cada uno necesita una elección.
  const conflicts = conflictsOf(mode === 'existing' ? tree : undefined, files, Number(season), episodes)
  const unresolved = files.filter((f) => conflicts[f.name] && !choices[f.name]).length
  const isShow = mode === 'existing' ? existing?.kind === 'show' : kind === 'show'

  function submit(event: FormEvent) {
    event.preventDefault()
    if (selectedLibrary === undefined) return
    const common = { title, year: year ? Number(year) : null }
    const choice = (name: string) => (conflicts[name] && choices[name]) || undefined
    const episodeFiles = files.map((file) => ({
      file: file.name,
      number: Number(episodes[file.name]),
      on_conflict: choice(file.name),
    }))
    let target: ImportTarget
    if (mode === 'new') {
      target =
        kind === 'movie'
          ? { kind: 'movie', ...common, file: files[0].name }
          : { kind: 'show', ...common, season: Number(season), episodes: episodeFiles }
    } else {
      if (!existing) return
      target =
        existing.kind === 'movie'
          ? { kind: 'existing_movie', movie_id: existing.id, file: files[0].name, on_conflict: choice(files[0].name) ?? null }
          : { kind: 'existing_show', show_id: existing.id, season: Number(season), episodes: episodeFiles }
    }
    onImport({ library_id: selectedLibrary, target })
  }

  if (libraries && libraries.length === 0) {
    return (
      <Alert tone="warning">
        No hay bibliotecas. <Link to="/libraries">Creá una</Link> antes de importar.
      </Alert>
    )
  }

  const seasons = tree?.children.map((s) => s.content.number).filter((n) => n !== null) ?? []

  return (
    <form onSubmit={submit}>
      <Card
        title="Destino"
        description="dónde y cómo queda en el catálogo"
        actions={
          <div className="segmented" role="group" aria-label="Destino">
            <button type="button" aria-pressed={mode === 'new'} onClick={() => setMode('new')}>
              Contenido nuevo
            </button>
            <button
              type="button"
              aria-pressed={mode === 'existing'}
              disabled={candidates.length === 0}
              title={candidates.length === 0 ? 'No hay nada en esta biblioteca todavía' : undefined}
              onClick={() => setMode('existing')}
            >
              Agregar a uno existente
            </button>
          </div>
        }
        footer={
          <>
            <button type="button" className="btn btn-ghost" onClick={onBack}>
              <ArrowLeft /> Elegir otros archivos
            </button>
            <span className="grow" />
            {unresolved > 0 && (
              <span className="small muted">
                Falta elegir qué hacer con {unresolved} archivo{unresolved === 1 ? '' : 's'}
              </span>
            )}
            <button
              className="btn btn-primary"
              type="submit"
              disabled={
                disabled || selectedLibrary === undefined || unresolved > 0 || (mode === 'existing' && !tree)
              }
            >
              Importar {files.length > 1 ? `${files.length} archivos` : ''}
            </button>
          </>
        }
      >
        <div className="card-body">
          <div className={`fields ${mode === 'existing' ? 'fields-existing' : ''}`}>
            <label className="field">
              Biblioteca
              <select
                className="select"
                value={selectedLibrary}
                onChange={(e) => setLibraryId(Number(e.target.value))}
              >
                {libraries?.map((library) => (
                  <option key={library.id} value={library.id}>
                    {library.name}
                  </option>
                ))}
              </select>
            </label>

            {mode === 'new' ? (
              <>
                <label className="field">
                  Tipo
                  <select
                    className="select"
                    value={kind}
                    onChange={(e) => setKind(e.target.value as 'movie' | 'show')}
                  >
                    <option value="show">Serie</option>
                    <option value="movie" disabled={!canBeMovie}>
                      Película {canBeMovie ? '' : '(un solo archivo)'}
                    </option>
                  </select>
                </label>
                <label className="field">
                  Título
                  <input className="input" value={title} onChange={(e) => setTitle(e.target.value)} required />
                </label>
                <label className="field">
                  Año
                  <input
                    className="input"
                    value={year}
                    placeholder="opcional"
                    onChange={(e) => setYear(e.target.value)}
                    inputMode="numeric"
                  />
                </label>
              </>
            ) : (
              <label className="field">
                {canBeMovie ? 'Película o serie' : 'Serie'}
                <select
                  className="select"
                  value={existingId ?? ''}
                  onChange={(e) => {
                    setExistingId(Number(e.target.value))
                    setChoices({})
                  }}
                  required
                >
                  <option value="" disabled>
                    Elegí…
                  </option>
                  {candidates.map((c) => (
                    <option key={c.id} value={c.id}>
                      {c.title}
                      {c.year ? ` (${c.year})` : ''} · {c.kind === 'show' ? 'serie' : 'película'}
                    </option>
                  ))}
                </select>
              </label>
            )}

            {isShow && (
              <label className="field">
                Temporada
                <input
                  className="input"
                  type="number"
                  min={0}
                  value={season}
                  onChange={(e) => setSeason(e.target.value)}
                  required
                />
                {mode === 'existing' && tree && (
                  <span className="field-hint">
                    {seasons.includes(Number(season))
                      ? `Temporada existente (tiene ${seasons.length === 1 ? 'solo esta' : `${seasons.join(', ')}`})`
                      : `Temporada nueva${seasons.length > 0 ? ` (hay ${seasons.join(', ')})` : ''}`}
                  </span>
                )}
              </label>
            )}
          </div>
        </div>

        <FileTable
          files={files}
          showEpisodes={isShow}
          episodes={episodes}
          onEpisode={(name, value) => setEpisodes({ ...episodes, [name]: value })}
          conflicts={conflicts}
          choices={choices}
          onChoose={(name, value) => setChoices({ ...choices, [name]: value })}
          onChooseAll={(value) =>
            setChoices(Object.fromEntries(files.filter((f) => conflicts[f.name]).map((f) => [f.name, value])))
          }
        />
      </Card>
    </form>
  )
}

/**
 * Para cada archivo cuyo destino ya tiene archivo, los nombres de esos archivos.
 * El destino es la película, o el episodio con ese número en esa temporada.
 */
function conflictsOf(
  tree: ContentDetail | undefined,
  files: FileAnalysis[],
  season: number,
  episodes: Record<string, string>,
): Record<string, string[]> {
  if (!tree) return {}
  const fileNames = (node: ContentDetail | undefined) => node?.files.map((f) => f.rel_path.split('/').pop()!) ?? []
  if (tree.content.kind === 'movie') {
    const current = fileNames(tree)
    return current.length > 0 ? { [files[0].name]: current } : {}
  }
  const seasonNode = tree.children.find((s) => s.content.number === season)
  const result: Record<string, string[]> = {}
  for (const file of files) {
    const episode = seasonNode?.children.find((e) => e.content.number === Number(episodes[file.name]))
    const current = fileNames(episode)
    if (current.length > 0) result[file.name] = current
  }
  return result
}

function FileTable({
  files,
  showEpisodes,
  episodes,
  onEpisode,
  conflicts,
  choices,
  onChoose,
  onChooseAll,
}: {
  files: FileAnalysis[]
  showEpisodes: boolean
  episodes: Record<string, string>
  onEpisode: (name: string, value: string) => void
  conflicts: Record<string, string[]>
  choices: Choices
  onChoose: (name: string, value: ConflictChoice) => void
  onChooseAll: (value: ConflictChoice) => void
}) {
  const conflictCount = Object.keys(conflicts).length
  if (!showEpisodes && conflictCount === 0) return null

  return (
    <>
      {conflictCount > 1 && (
        <div className="card-body conflict-bar">
          <TriangleAlert size={16} />
          <span className="grow">
            {conflictCount} episodios ya tienen archivo. Elegí qué hacer con cada uno, o con todos:
          </span>
          <select
            className="select"
            value=""
            onChange={(e) => onChooseAll(e.target.value as ConflictChoice)}
            aria-label="Elegir para todos"
          >
            <option value="" disabled>
              Para todos…
            </option>
            {Object.entries(CHOICE_LABELS).map(([value, label]) => (
              <option key={value} value={value}>
                {label}
              </option>
            ))}
          </select>
        </div>
      )}
      <table className="table compact">
        <thead>
          <tr>
            <th>Archivo</th>
            {showEpisodes && <th style={{ width: 110 }}>Episodio</th>}
            <th style={{ width: 300 }}>Destino</th>
          </tr>
        </thead>
        <tbody>
          {files.map((file) => {
            const current = conflicts[file.name]
            return (
              <tr key={file.name}>
                <td className="mono">{file.name}</td>
                {showEpisodes && (
                  <td>
                    <input
                      className="input input-narrow"
                      type="number"
                      min={0}
                      value={episodes[file.name]}
                      onChange={(e) => onEpisode(file.name, e.target.value)}
                      required
                    />
                  </td>
                )}
                <td>
                  {current ? (
                    <div className="conflict-cell">
                      <span className="small" title={current.join('\n')}>
                        <Badge tone="warning" icon={TriangleAlert}>
                          Ya tiene archivo
                        </Badge>
                      </span>
                      <select
                        className="select"
                        value={choices[file.name] ?? ''}
                        onChange={(e) => onChoose(file.name, e.target.value as ConflictChoice)}
                        required
                        aria-label={`Qué hacer con ${file.name}`}
                      >
                        <option value="" disabled>
                          Elegí qué hacer…
                        </option>
                        {Object.entries(CHOICE_LABELS).map(([value, label]) => (
                          <option key={value} value={value}>
                            {label}
                          </option>
                        ))}
                      </select>
                    </div>
                  ) : (
                    <span className="small muted">Nuevo</span>
                  )}
                </td>
              </tr>
            )
          })}
        </tbody>
      </table>
    </>
  )
}

// --- Resultado ---

const ACTION_LABELS: Record<ImportAction, string> = {
  added: 'Importado',
  replaced: 'Reemplazado',
  added_version: 'Otra versión',
  skipped: 'Omitido',
}

function ResultView({ result, onRestart }: { result: ImportResult; onRestart: () => void }) {
  const failed = result.files.filter((file) => file.error).length
  const trashed = result.files.some((file) => file.trashed.length > 0)
  const queued = result.files.some((file) => file.job_id !== null)
  return (
    <Card
      title={
        failed > 0
          ? `Importación con ${failed} error(es)`
          : queued
            ? 'Importación en marcha'
            : 'Importación completa'
      }
      actions={
        <>
          {queued && (
            <Link to="/jobs" className="btn">
              Ver la cola
            </Link>
          )}
          <button className="btn btn-primary" onClick={onRestart}>
            Importar otra cosa
          </button>
        </>
      }
      footer={
        <span className="small muted">
          {trashed && (
            <>
              Los archivos reemplazados quedaron en <span className="mono">.kinora-trash/</span> dentro de la
              biblioteca.{' '}
            </>
          )}
          Para depurar: <Link to="/database/contents">contents</Link> ·{' '}
          <Link to="/database/media_files">media_files</Link>
        </span>
      }
    >
      {result.files.map((file) => (
        <div key={file.file} className="list-row">
          {file.error ? (
            <>
              <CircleX className="icon" color="var(--critical)" />
              <span className="grow">
                <span className="mono">{file.file}</span>
                <div className="small" style={{ color: 'var(--critical-text)' }}>
                  {file.error}
                </div>
              </span>
            </>
          ) : file.action === 'skipped' ? (
            <>
              <SkipForward className="icon" />
              <span className="grow">
                <span className="mono">{file.file}</span>
                <div className="small muted">Sigue en la bandeja de entrada</div>
              </span>
              <Badge>{ACTION_LABELS.skipped}</Badge>
            </>
          ) : file.job_id !== null ? (
            <>
              <Clock className="icon" />
              <span className="grow">
                <span className="mono">{file.destination}</span>
                <div className="small muted">
                  Necesita remux: entra a la biblioteca cuando termine.
                </div>
              </span>
              <Link to="/jobs" className="badge">
                En cola
              </Link>
            </>
          ) : (
            <>
              <CircleCheck className="icon" color="var(--good)" />
              <span className="mono grow">{file.destination}</span>
              {file.subtitles > 0 && (
                <Badge icon={Captions}>
                  {file.subtitles} subtítulo{file.subtitles === 1 ? '' : 's'}
                </Badge>
              )}
              <Badge tone={file.action === 'added' ? 'good' : undefined}>{ACTION_LABELS[file.action]}</Badge>
            </>
          )}
        </div>
      ))}
    </Card>
  )
}
