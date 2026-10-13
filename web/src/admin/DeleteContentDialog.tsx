import { useState } from 'react'
import { api } from '../shared/api'
import { formatSize } from '../shared/format'
import type { ContentSummary } from '../shared/types/ContentSummary'
import { Alert, ConfirmDialog } from './ui'

/** Confirmación para eliminar una película o serie, diciendo exactamente qué se va. */
export function DeleteContentDialog({
  summary,
  onClose,
  onDeleted,
}: {
  summary: ContentSummary
  onClose: () => void
  onDeleted: () => void
}) {
  const [deleting, setDeleting] = useState(false)
  const [error, setError] = useState<string>()
  const { content } = summary
  const blocked = summary.active_jobs > 0

  async function confirm() {
    setDeleting(true)
    setError(undefined)
    try {
      await api(`/api/admin/contents/${content.id}`, { method: 'DELETE' })
      onDeleted()
    } catch (err) {
      setError((err as Error).message)
      setDeleting(false)
    }
  }

  const what =
    content.kind === 'show'
      ? `la serie con sus ${summary.seasons === 1 ? 'temporada' : `${summary.seasons} temporadas`} y ${summary.episodes} episodio${summary.episodes === 1 ? '' : 's'}`
      : 'la película'

  return (
    <ConfirmDialog
      title={`¿Eliminar ${content.title}?`}
      confirmLabel="Eliminar"
      danger
      busy={deleting}
      disabled={blocked}
      onConfirm={confirm}
      onCancel={onClose}
    >
      {blocked ? (
        <Alert tone="warning">
          Tiene {summary.active_jobs} archivo{summary.active_jobs === 1 ? '' : 's'} en la cola. Cancelalos (o esperá a
          que terminen) antes de eliminarla.
        </Alert>
      ) : (
        <>
          <p>Se saca del catálogo {what}.</p>
          {summary.files > 0 ? (
            <ul className="dialog-list">
              <li>
                <strong>
                  {summary.files} archivo{summary.files === 1 ? '' : 's'} de video ({formatSize(summary.size)})
                </strong>
                {summary.subtitles > 0 && ` y ${summary.subtitles} subtítulo${summary.subtitles === 1 ? '' : 's'}`}: la
                carpeta <span className="mono">{content.dir}</span> va a la papelera de la biblioteca
                (<span className="mono">.kinora-trash</span>), de donde se puede recuperar a mano.
              </li>
              <li>El progreso (por dónde ibas) se borra.</li>
              <li>El historial de lo visto se conserva.</li>
            </ul>
          ) : (
            <p className="muted">No tiene archivos: solo se borra del catálogo.</p>
          )}
        </>
      )}
      {error && <Alert>{error}</Alert>}
    </ConfirmDialog>
  )
}
