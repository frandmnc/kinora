// Textos que comparten varias pantallas del Admin.

import type { Operation } from '../shared/types/Operation'

/** Qué le hace Kinora a un archivo para llevarlo al formato de la casa, en pocas palabras. */
export const OPERATION_LABELS: Record<Operation, string> = {
  copy: 'Se mueve tal cual',
  remux: 'Remux',
  audio: 'Remux + audio a AAC',
  video: 'Conversión',
}
