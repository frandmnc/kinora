// Buscar subtítulos en OpenSubtitles para un video y agregar el elegido.

import { useEffect, useState } from 'react'
import { Link } from 'react-router'
import { Check, Download, LoaderCircle, ShieldCheck, X } from 'lucide-react'
import { api } from '../shared/api'
import { useApi } from '../shared/useApi'
import type { DownloadSubtitle } from '../shared/types/DownloadSubtitle'
import type { DownloadedSubtitle } from '../shared/types/DownloadedSubtitle'
import type { OpenSubtitlesSetting } from '../shared/types/OpenSubtitlesSetting'
import type { SubtitleCandidate } from '../shared/types/SubtitleCandidate'
import type { SubtitleLanguage } from '../shared/types/SubtitleLanguage'
import type { SubtitleSearch as SearchResult } from '../shared/types/SubtitleSearch'
import { Alert } from './ui'

/** Los idiomas que se ofrecen para buscar, en este orden (si OpenSubtitles los tiene). */
const OFFERED: [code: string, name: string][] = [
  ['es', 'Español'],
  ['ea', 'Español latino'],
  ['en', 'Inglés'],
  ['pt-br', 'Portugués (Brasil)'],
]
/** Los que se buscan si nunca se eligió otra cosa. */
const DEFAULT = ['es', 'ea']
/** Dónde se recuerdan los idiomas elegidos, para la próxima búsqueda. */
const PREFERENCE = 'kinora.subtitle-languages'
/** Cuántos resultados se ven antes de "Ver más". */
const FIRST = 10

function readPreference(): string[] {
  try {
    const saved = localStorage.getItem(PREFERENCE)
    return saved ? saved.split(',').filter(Boolean) : DEFAULT
  } catch {
    return DEFAULT
  }
}

function savePreference(languages: string[]) {
  try {
    localStorage.setItem(PREFERENCE, languages.join(','))
  } catch {
    // Sin almacenamiento: la elección vale solo para esta vez.
  }
}

export function SubtitleSearch({
  fileId,
  onAdded,
  onClose,
}: {
  fileId: number
  onAdded: () => void
  onClose: () => void
}) {
  const { data: setting } = useApi<OpenSubtitlesSetting>('/api/admin/settings/opensubtitles')
  const configured = setting?.configured ?? false
  const { data: languages, error: languagesError } = useApi<SubtitleLanguage[]>(
    configured ? '/api/admin/opensubtitles/languages' : '',
  )
  const [chosen, setChosen] = useState<string[]>(readPreference)
  const [search, setSearch] = useState<SearchResult>()
  const [searching, setSearching] = useState(false)
  const [error, setError] = useState<string>()
  const [showAll, setShowAll] = useState(false)
  const [adding, setAdding] = useState<number>()
  const [added, setAdded] = useState<number[]>([])

  const offered: SubtitleLanguage[] = OFFERED.filter(([code]) => languages?.some((l) => l.code === code)).map(
    ([code, name]) => ({ code, name }),
  )
  const query = chosen.filter((code) => offered.some((l) => l.code === code)).join(',')

  useEffect(() => {
    if (!configured || !query) return
    let cancelled = false
    setSearching(true)
    setError(undefined)
    setShowAll(false)
    api<SearchResult>(`/api/admin/files/${fileId}/subtitles/search?languages=${encodeURIComponent(query)}`)
      .then((result) => !cancelled && setSearch(result))
      .catch((err: Error) => !cancelled && setError(err.message))
      .finally(() => !cancelled && setSearching(false))
    return () => {
      cancelled = true
    }
  }, [configured, fileId, query])

  function toggle(code: string) {
    const next = chosen.includes(code) ? chosen.filter((c) => c !== code) : [...chosen, code]
    setChosen(next)
    savePreference(next)
  }

  async function add(candidate: SubtitleCandidate) {
    setAdding(candidate.file_id)
    setError(undefined)
    try {
      const body: DownloadSubtitle = {
        file_id: candidate.file_id,
        language: candidate.language,
        hearing_impaired: candidate.hearing_impaired,
        forced: candidate.forced,
        release: candidate.release,
      }
      const result = await api<DownloadedSubtitle>(`/api/admin/files/${fileId}/subtitles/opensubtitles`, {
        method: 'POST',
        body,
      })
      setAdded([...added, candidate.file_id])
      if (search) setSearch({ ...search, remaining: result.remaining })
      onAdded()
    } catch (err) {
      setError((err as Error).message)
    } finally {
      setAdding(undefined)
    }
  }

  if (setting && !configured) {
    return (
      <div className="subtitle-search">
        <Alert tone="info">
          Para buscar subtítulos hace falta una cuenta de OpenSubtitles: cargala en{' '}
          <Link to="/settings">Configuración</Link>.
        </Alert>
        <button className="btn btn-ghost btn-sm" onClick={onClose}>
          Cerrar
        </button>
      </div>
    )
  }

  const results = search?.results ?? []
  const shown = showAll ? results : results.slice(0, FIRST)

  return (
    <div className="subtitle-search">
      <div className="subtitle-search-head">
        <div className="lang-chips">
          {offered.map((language) => (
            <button
              key={language.code}
              type="button"
              className={`lang-chip ${chosen.includes(language.code) ? 'selected' : ''}`}
              aria-pressed={chosen.includes(language.code)}
              onClick={() => toggle(language.code)}
            >
              {chosen.includes(language.code) && <Check />}
              {language.name}
            </button>
          ))}
        </div>
        <span className="small muted grow">
          {search?.remaining != null && `Te quedan ${search.remaining} descargas hoy`}
        </span>
        <button className="btn btn-ghost btn-sm btn-icon" onClick={onClose} title="Cerrar" aria-label="Cerrar">
          <X />
        </button>
      </div>

      {(error || languagesError) && <Alert>{error ?? languagesError}</Alert>}
      {!query && offered.length > 0 && <p className="small muted">Elegí al menos un idioma.</p>}
      {searching && (
        <p className="small muted row">
          <LoaderCircle className="spin" size={14} /> Buscando en OpenSubtitles…
        </p>
      )}
      {!searching && search && search.results.length === 0 && (
        <p className="small muted">
          No hay subtítulos en esos idiomas
          {search.by_tmdb ? '.' : '. Se buscó por el título; con la metadata de TMDB la búsqueda es más precisa.'}
        </p>
      )}

      {!searching && shown.length > 0 && (
        <ul className="subtitle-results">
          {shown.map((candidate) => {
            const done = added.includes(candidate.file_id)
            return (
              <li key={candidate.file_id} className="subtitle-result">
                <div className="grow subtitle-result-text">
                  <div className="row">
                    <strong>{candidate.label}</strong>
                    {candidate.trusted && (
                      <span className="badge badge-good" title="Subido por alguien de confianza de OpenSubtitles">
                        <ShieldCheck /> Confiable
                      </span>
                    )}
                    {candidate.machine_translated && (
                      <span className="badge badge-warning" title="Traducido por una máquina: suele tener errores">
                        Traducción automática
                      </span>
                    )}
                  </div>
                  <div className="mono small truncate" title={candidate.release}>
                    {candidate.release}
                  </div>
                  <div className="small muted">
                    {candidate.download_count.toLocaleString('es-AR')} descargas
                    {candidate.uploader && ` · ${candidate.uploader}`}
                  </div>
                </div>
                <button
                  className="btn btn-sm"
                  disabled={done || adding !== undefined}
                  onClick={() => add(candidate)}
                >
                  {done ? (
                    <>
                      <Check /> Agregado
                    </>
                  ) : adding === candidate.file_id ? (
                    <>
                      <LoaderCircle className="spin" /> Bajando…
                    </>
                  ) : (
                    <>
                      <Download /> Agregar
                    </>
                  )}
                </button>
              </li>
            )
          })}
        </ul>
      )}
      {!searching && !showAll && results.length > FIRST && (
        <button className="link-button small" onClick={() => setShowAll(true)}>
          Ver los {results.length - FIRST} restantes
        </button>
      )}
    </div>
  )
}
