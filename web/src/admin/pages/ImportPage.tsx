import { useEffect, useState, type DragEvent, type FormEvent } from 'react'
import { Link } from 'react-router'
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

  return (
    <>
      <h1>Importar</h1>
      <p className="muted">
        Por ahora solo se importan archivos que ya están en el formato de Kinora (MP4, HEVC, AAC), como contenido
        nuevo. La conversión llega en la Etapa 4.
      </p>

      {busy && <p className="notice">{busy}</p>}
      {error && <p className="error">{error}</p>}

      {result ? (
        <ResultView result={result} onRestart={restart} />
      ) : analysis ? (
        <>
          <AnalysisTable files={analysis} />
          {analysis.every(isReady) ? (
            <DestinationForm files={analysis} disabled={!!busy} onImport={importFiles} />
          ) : (
            <p className="muted">Hay archivos que no se pueden importar todavía. Volvé y elegí otros.</p>
          )}
          <button onClick={restart}>← Elegir otros archivos</button>
        </>
      ) : (
        <Inbox disabled={!!busy} onAnalyze={analyze} />
      )}
    </>
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
      <p className="error">
        Primero elegí la carpeta media en <Link to="/settings">Configuración</Link>.
      </p>
    )
  }

  return (
    <>
      <h2>1. Subí los archivos</h2>
      <label
        className={`dropzone ${dragging ? 'dragging' : ''}`}
        onDragOver={(e) => {
          e.preventDefault()
          setDragging(true)
        }}
        onDragLeave={() => setDragging(false)}
        onDrop={onDrop}
      >
        <strong>Soltá acá videos o una carpeta</strong>
        <span className="muted">o hacé clic para elegirlos</span>
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

      {uploads.length > 0 && (
        <ul className="uploads">
          {uploads.map((u) => (
            <li key={u.name}>
              <span className="mono grow">{u.name}</span>
              {u.error ? (
                <span className="error">{u.error}</span>
              ) : u.sent >= u.size ? (
                <span className="ok">✓ subido</span>
              ) : (
                <>
                  <span className="bar">
                    <span style={{ width: `${(u.sent / u.size) * 100}%` }} />
                  </span>
                  <span className="muted">
                    {formatSize(u.sent)} / {formatSize(u.size)}
                  </span>
                </>
              )}
            </li>
          ))}
        </ul>
      )}

      <div className="page-header">
        <h2>Bandeja de entrada</h2>
        <button
          className="primary"
          disabled={disabled || uploading || selected.size === 0}
          onClick={() => onAnalyze([...selected])}
        >
          Analizar {selected.size > 0 && `(${selected.size})`}
        </button>
      </div>
      {error && <p className="error">{error}</p>}
      {inbox?.length === 0 && <p className="muted">La bandeja está vacía.</p>}
      {inbox?.map((file) => (
        <label key={file.name} className="file-row">
          <input type="checkbox" checked={selected.has(file.name)} onChange={() => toggle(file.name)} />
          <span className="mono grow">{file.name}</span>
          <span className="muted">{formatSize(file.size)}</span>
          <button
            className="danger"
            onClick={(e) => {
              e.preventDefault()
              if (confirm(`¿Descartar ${file.name}? Se borra de la bandeja.`)) discard(file.name)
            }}
          >
            Descartar
          </button>
        </label>
      ))}
    </>
  )
}

// --- Paso 2: análisis y destino ---

function AnalysisTable({ files }: { files: FileAnalysis[] }) {
  return (
    <>
      <h2>2. Análisis</h2>
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
              <td className="mono">
                {file.name}
                <div className="muted">{formatSize(file.size)}</div>
              </td>
              <td>{formatDuration(file.duration)}</td>
              <td>{file.video ?? '—'}</td>
              <td>{file.audio.join(', ') || '—'}</td>
              <td>
                {file.error ? (
                  <span className="error">{file.error}</span>
                ) : file.problems.length === 0 ? (
                  <span className="ok">Listo para copiar</span>
                ) : (
                  <ul className="problems">
                    {file.problems.map((problem) => (
                      <li key={problem}>{problem}</li>
                    ))}
                  </ul>
                )}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </>
  )
}

function DestinationForm({
  files,
  disabled,
  onImport,
}: {
  files: FileAnalysis[]
  disabled: boolean
  onImport: (request: ImportRequest) => void
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
      <p className="error">
        No hay bibliotecas. <Link to="/libraries">Creá una</Link> antes de importar.
      </p>
    )
  }

  return (
    <form className="destination" onSubmit={submit}>
      <h2>3. Destino</h2>
      <div className="fields">
        <label>
          Biblioteca
          <select value={selectedLibrary} onChange={(e) => setLibraryId(Number(e.target.value))}>
            {libraries?.map((library) => (
              <option key={library.id} value={library.id}>
                {library.name}
              </option>
            ))}
          </select>
        </label>
        <label>
          Tipo
          <select value={kind} onChange={(e) => setKind(e.target.value as 'movie' | 'show')}>
            <option value="show">Serie</option>
            <option value="movie" disabled={!canBeMovie}>
              Película {canBeMovie ? '' : '(un solo archivo)'}
            </option>
          </select>
        </label>
        <label className="grow">
          Título
          <input value={title} onChange={(e) => setTitle(e.target.value)} required />
        </label>
        <label>
          Año
          <input className="narrow" value={year} onChange={(e) => setYear(e.target.value)} inputMode="numeric" />
        </label>
        {kind === 'show' && (
          <label>
            Temporada
            <input
              className="narrow"
              type="number"
              min={0}
              value={season}
              onChange={(e) => setSeason(e.target.value)}
              required
            />
          </label>
        )}
      </div>

      {kind === 'show' && (
        <table className="table compact">
          <thead>
            <tr>
              <th>Archivo</th>
              <th>Episodio</th>
            </tr>
          </thead>
          <tbody>
            {files.map((file) => (
              <tr key={file.name}>
                <td className="mono">{file.name}</td>
                <td>
                  <input
                    className="narrow"
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

      <button className="primary" type="submit" disabled={disabled || selectedLibrary === undefined}>
        Importar
      </button>
    </form>
  )
}

// --- Resultado ---

function ResultView({ result, onRestart }: { result: ImportResult; onRestart: () => void }) {
  const failed = result.files.filter((file) => file.error).length
  return (
    <>
      <h2>{failed === 0 ? 'Importación completa' : `Importación con ${failed} error(es)`}</h2>
      <ul className="results">
        {result.files.map((file) => (
          <li key={file.file}>
            {file.error ? (
              <span className="error">✗ {file.file}: {file.error}</span>
            ) : (
              <span className="ok">✓ {file.destination}</span>
            )}
          </li>
        ))}
      </ul>
      <p>
        <Link to="/database/contents">Ver contents</Link> · <Link to="/database/media_files">Ver media_files</Link>
      </p>
      <button onClick={onRestart}>Importar otra cosa</button>
    </>
  )
}
