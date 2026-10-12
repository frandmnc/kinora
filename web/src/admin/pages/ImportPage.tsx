import { Fragment, useEffect, useState, type DragEvent, type FormEvent } from 'react'
import { Link } from 'react-router'
import {
  ArrowLeft,
  Check,
  CircleCheck,
  CircleX,
  CloudUpload,
  FilePlay,
  Inbox as InboxIcon,
  LoaderCircle,
  Trash,
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
import { Alert, Badge, Card, EmptyState, Meter, OkBadge, PageHeader } from '../ui'

/**
 * Importar en tres pasos: subir archivos a la bandeja → revisar el análisis y elegir el destino → ver el resultado.
 * Etapa 1: solo archivos que ya están en el formato de Kinora, y siempre como contenido nuevo.
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

  return (
    <>
      <PageHeader
        title="Importar"
        description="Por ahora solo se importan archivos que ya están en el formato de Kinora (MP4, HEVC, AAC), como contenido nuevo. La conversión llega en la Etapa 4."
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
          {analysis.every(isReady) ? (
            <DestinationForm files={analysis} disabled={!!busy} onImport={importFiles} onBack={restart} />
          ) : (
            <Alert tone="warning">
              Hay archivos que todavía no se pueden importar.{' '}
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
  return !file.error && file.problems.length === 0
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
                <td>
                  {file.error ? (
                    <Badge tone="critical" icon={CircleX}>
                      No se pudo leer
                    </Badge>
                  ) : file.problems.length === 0 ? (
                    <OkBadge>Listo para importar</OkBadge>
                  ) : (
                    <ul className="problems">
                      {file.problems.map((problem) => (
                        <li key={problem}>
                          <Badge tone="warning" icon={CircleX}>
                            {problem}
                          </Badge>
                        </li>
                      ))}
                    </ul>
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
  const [kind, setKind] = useState<'movie' | 'show'>(canBeMovie && first.episode === null ? 'movie' : 'show')
  const [title, setTitle] = useState(first.title ?? '')
  const [year, setYear] = useState(first.year?.toString() ?? '')
  const [season, setSeason] = useState(String(first.season ?? 1))
  const [episodes, setEpisodes] = useState<Record<string, string>>(() =>
    Object.fromEntries(files.map((file, i) => [file.name, String(file.guess.episode ?? i + 1)])),
  )

  const selectedLibrary = libraryId ?? libraries?.[0]?.id

  function submit(event: FormEvent) {
    event.preventDefault()
    if (selectedLibrary === undefined) return
    const common = { title, year: year ? Number(year) : null }
    onImport({
      library_id: selectedLibrary,
      target:
        kind === 'movie'
          ? { kind: 'movie', ...common, file: files[0].name }
          : {
              kind: 'show',
              ...common,
              season: Number(season),
              episodes: files.map((file) => ({ file: file.name, number: Number(episodes[file.name]) })),
            },
    })
  }

  if (libraries && libraries.length === 0) {
    return (
      <Alert tone="warning">
        No hay bibliotecas. <Link to="/libraries">Creá una</Link> antes de importar.
      </Alert>
    )
  }

  return (
    <form onSubmit={submit}>
      <Card
        title="Destino"
        description="dónde y cómo queda en el catálogo"
        footer={
          <>
            <button type="button" className="btn btn-ghost" onClick={onBack}>
              <ArrowLeft /> Elegir otros archivos
            </button>
            <span className="grow" />
            <button className="btn btn-primary" type="submit" disabled={disabled || selectedLibrary === undefined}>
              Importar {files.length > 1 ? `${files.length} archivos` : ''}
            </button>
          </>
        }
      >
        <div className="card-body">
          <div className="fields">
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
            <label className="field">
              Tipo
              <select className="select" value={kind} onChange={(e) => setKind(e.target.value as 'movie' | 'show')}>
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
            {kind === 'show' && (
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
              </label>
            )}
          </div>
        </div>

        {kind === 'show' && (
          <table className="table compact">
            <thead>
              <tr>
                <th>Archivo</th>
                <th style={{ width: 130 }}>Episodio</th>
              </tr>
            </thead>
            <tbody>
              {files.map((file) => (
                <tr key={file.name}>
                  <td className="mono">{file.name}</td>
                  <td>
                    <input
                      className="input input-narrow"
                      type="number"
                      min={0}
                      value={episodes[file.name]}
                      onChange={(e) => setEpisodes({ ...episodes, [file.name]: e.target.value })}
                      required
                    />
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </Card>
    </form>
  )
}

// --- Resultado ---

function ResultView({ result, onRestart }: { result: ImportResult; onRestart: () => void }) {
  const failed = result.files.filter((file) => file.error).length
  return (
    <Card
      title={failed === 0 ? 'Importación completa' : `Importación con ${failed} error(es)`}
      actions={
        <button className="btn btn-primary" onClick={onRestart}>
          Importar otra cosa
        </button>
      }
      footer={
        <span className="small muted">
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
          ) : (
            <>
              <CircleCheck className="icon" color="var(--good)" />
              <span className="mono grow">{file.destination}</span>
            </>
          )}
        </div>
      ))}
    </Card>
  )
}
