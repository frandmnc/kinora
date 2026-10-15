// La metadata de una película o serie: carátula, título, año y sinopsis. Se busca en TMDB o se
// corrige a mano.

import { useState, type FormEvent } from 'react'
import { Link } from 'react-router'
import { ExternalLink, ImageOff, LoaderCircle, Pencil, RefreshCw, Search } from 'lucide-react'
import { api, mediaUrl } from '../shared/api'
import { useApi } from '../shared/useApi'
import type { ApplyMetadata } from '../shared/types/ApplyMetadata'
import type { Content } from '../shared/types/Content'
import type { ContentChanges } from '../shared/types/ContentChanges'
import type { MetadataCandidate } from '../shared/types/MetadataCandidate'
import type { TmdbSetting } from '../shared/types/TmdbSetting'
import { Alert, Card } from './ui'

type Mode = 'view' | 'search' | 'edit'

export function MetadataCard({ content, onChange }: { content: Content; onChange: () => void }) {
  const { data: tmdb } = useApi<TmdbSetting>('/api/admin/settings/tmdb')
  const [mode, setMode] = useState<Mode>('view')
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string>()

  /** Usa un resultado de TMDB (o vuelve a pedir el que ya tiene). Tarda: baja las imágenes. */
  async function apply(tmdbId: number) {
    setBusy(true)
    setError(undefined)
    try {
      const body: ApplyMetadata = { tmdb_id: tmdbId }
      await api(`/api/admin/contents/${content.id}/metadata`, { method: 'POST', body })
      setMode('view')
      onChange()
    } catch (err) {
      setError((err as Error).message)
    } finally {
      setBusy(false)
    }
  }

  const tmdbKind = content.kind === 'show' ? 'tv' : 'movie'

  return (
    <Card
      title="Metadata"
      description={
        content.metadata_at
          ? `de TMDB, traída ${new Date(content.metadata_at * 1000).toLocaleDateString('es-AR', { day: 'numeric', month: 'short', year: 'numeric' })}`
          : 'cargada a mano o sacada del nombre del archivo'
      }
      actions={
        mode === 'view' && (
          <>
            {content.tmdb_id && tmdb?.configured && (
              <button className="btn btn-sm" onClick={() => apply(content.tmdb_id!)} disabled={busy}>
                {busy ? <LoaderCircle className="spin" /> : <RefreshCw />} Actualizar
              </button>
            )}
            <button className="btn btn-sm" onClick={() => setMode('edit')} disabled={busy}>
              <Pencil /> Editar
            </button>
            <button className="btn btn-sm btn-primary" onClick={() => setMode('search')} disabled={busy}>
              <Search /> Buscar en TMDB
            </button>
          </>
        )
      }
    >
      <div className="card-body metadata">
        <Artwork content={content} />
        <div className="metadata-text">
          {mode === 'edit' ? (
            <EditForm
              content={content}
              onDone={() => {
                setMode('view')
                onChange()
              }}
              onCancel={() => setMode('view')}
            />
          ) : (
            <>
              <h3>
                {content.title}
                {content.year && <span className="muted"> ({content.year})</span>}
              </h3>
              {content.overview ? (
                <p className="metadata-overview">{content.overview}</p>
              ) : (
                <p className="muted">Sin sinopsis.</p>
              )}
              {content.tmdb_id && (
                <a
                  className="small"
                  href={`https://www.themoviedb.org/${tmdbKind}/${content.tmdb_id}`}
                  target="_blank"
                  rel="noreferrer"
                >
                  Ver en TMDB <ExternalLink className="inline-icon" />
                </a>
              )}
            </>
          )}
          {error && <Alert>{error}</Alert>}
        </div>
      </div>

      {mode === 'search' &&
        (tmdb && !tmdb.configured ? (
          <div className="card-body">
            <Alert tone="info">
              Para buscar en TMDB hace falta su clave: cargala en <Link to="/settings">Configuración</Link>.
            </Alert>
            <button className="btn btn-ghost" onClick={() => setMode('view')}>
              Cerrar
            </button>
          </div>
        ) : (
          <SearchPanel content={content} busy={busy} onPick={apply} onCancel={() => setMode('view')} />
        ))}
    </Card>
  )
}

/** La carátula, o un recuadro vacío. El fondo, si hay, detrás. */
function Artwork({ content }: { content: Content }) {
  return (
    <div className="metadata-art">
      {content.poster_at ? (
        <img src={mediaUrl(`/api/contents/${content.id}/poster?v=${content.poster_at}`)} alt="" />
      ) : (
        <div className="metadata-no-art">
          <ImageOff />
          Sin carátula
        </div>
      )}
    </div>
  )
}

function SearchPanel({
  content,
  busy,
  onPick,
  onCancel,
}: {
  content: Content
  busy: boolean
  onPick: (tmdbId: number) => void
  onCancel: () => void
}) {
  const [query, setQuery] = useState(content.title)
  const [year, setYear] = useState(content.year ? String(content.year) : '')
  const [results, setResults] = useState<MetadataCandidate[]>()
  const [searching, setSearching] = useState(false)
  const [error, setError] = useState<string>()
  const [picked, setPicked] = useState<number>()

  async function search(event?: FormEvent) {
    event?.preventDefault()
    setSearching(true)
    setError(undefined)
    try {
      const params = new URLSearchParams({ q: query })
      if (year.trim()) params.set('year', year.trim())
      setResults(await api<MetadataCandidate[]>(`/api/admin/contents/${content.id}/metadata/search?${params}`))
    } catch (err) {
      setError((err as Error).message)
    } finally {
      setSearching(false)
    }
  }

  return (
    <div className="metadata-search">
      <form className="metadata-search-form" onSubmit={search}>
        <label className="field grow">
          Título
          <input className="input" value={query} onChange={(e) => setQuery(e.target.value)} autoFocus />
        </label>
        <label className="field">
          Año
          <input
            className="input input-narrow"
            value={year}
            onChange={(e) => setYear(e.target.value.replace(/\D/g, '').slice(0, 4))}
            inputMode="numeric"
            placeholder="opcional"
          />
        </label>
        <button className="btn btn-primary" type="submit" disabled={searching || !query.trim()}>
          {searching ? <LoaderCircle className="spin" /> : <Search />} Buscar
        </button>
        <button className="btn btn-ghost" type="button" onClick={onCancel} disabled={busy}>
          Cerrar
        </button>
      </form>

      {error && <Alert>{error}</Alert>}
      {results?.length === 0 && <p className="muted">TMDB no encontró nada con ese título. Probá en inglés o sin el año.</p>}
      {results && results.length > 0 && (
        <ul className="metadata-results">
          {results.map((result) => (
            <li key={result.tmdb_id} className="metadata-result">
              {result.thumbnail ? (
                <img src={result.thumbnail} alt="" loading="lazy" />
              ) : (
                <div className="metadata-result-empty">
                  <ImageOff />
                </div>
              )}
              <div className="grow">
                <strong>{result.title}</strong>
                {result.year && <span className="muted"> ({result.year})</span>}
                {result.original_title && <div className="small muted">{result.original_title}</div>}
                {result.overview && <p className="metadata-result-overview small">{result.overview}</p>}
              </div>
              <button
                className="btn btn-sm"
                disabled={busy}
                onClick={() => {
                  setPicked(result.tmdb_id)
                  onPick(result.tmdb_id)
                }}
              >
                {busy && picked === result.tmdb_id ? (
                  <>
                    <LoaderCircle className="spin" /> Trayendo…
                  </>
                ) : result.tmdb_id === content.tmdb_id ? (
                  'Es el actual'
                ) : (
                  'Usar este'
                )}
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  )
}

function EditForm({ content, onDone, onCancel }: { content: Content; onDone: () => void; onCancel: () => void }) {
  const [title, setTitle] = useState(content.title)
  const [year, setYear] = useState(content.year ? String(content.year) : '')
  const [overview, setOverview] = useState(content.overview ?? '')
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string>()

  async function save(event: FormEvent) {
    event.preventDefault()
    setBusy(true)
    setError(undefined)
    try {
      const body: ContentChanges = { title, year: year ? Number(year) : null, overview: overview || null }
      await api(`/api/admin/contents/${content.id}`, { method: 'PATCH', body })
      onDone()
    } catch (err) {
      setError((err as Error).message)
      setBusy(false)
    }
  }

  return (
    <form className="metadata-edit" onSubmit={save}>
      <div className="metadata-edit-row">
        <label className="field grow">
          Título
          <input className="input" value={title} onChange={(e) => setTitle(e.target.value)} required autoFocus />
        </label>
        <label className="field">
          Año
          <input
            className="input input-narrow"
            value={year}
            onChange={(e) => setYear(e.target.value.replace(/\D/g, '').slice(0, 4))}
            inputMode="numeric"
          />
        </label>
      </div>
      <label className="field">
        Sinopsis
        <textarea className="input" rows={5} value={overview} onChange={(e) => setOverview(e.target.value)} />
      </label>
      <p className="small muted">La carpeta y los archivos conservan su nombre: solo cambia lo que se muestra.</p>
      {error && <Alert>{error}</Alert>}
      <div className="row">
        <button className="btn btn-primary" type="submit" disabled={busy || !title.trim()}>
          Guardar
        </button>
        <button className="btn btn-ghost" type="button" onClick={onCancel} disabled={busy}>
          Cancelar
        </button>
      </div>
    </form>
  )
}
