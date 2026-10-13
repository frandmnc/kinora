// Versiones: un mismo episodio o película puede tener varios archivos (por ejemplo, 1080p y 4K).

import { formatSize } from './format'
import type { MediaFile } from './types/MediaFile'

/**
 * El archivo a reproducir: el pedido, si existe; si no, el de mayor resolución
 * (y entre iguales, el agregado más recientemente).
 */
export function pickFile(files: MediaFile[], requested?: number | null): MediaFile | undefined {
  const chosen = requested != null ? files.find((f) => f.id === requested) : undefined
  return chosen ?? [...files].sort((a, b) => (b.height ?? 0) - (a.height ?? 0) || b.id - a.id)[0]
}

/** "2160p", "1080p", "720p"… a partir del alto de la imagen. */
export function resolution(file: MediaFile) {
  const height = file.height ?? 0
  if (height >= 2000) return '2160p'
  if (height >= 1000) return '1080p'
  if (height >= 700) return '720p'
  return height > 0 ? `${height}p` : 'Video'
}

/** "1080p · 1,2 GB", para elegir entre versiones. */
export function versionLabel(file: MediaFile) {
  return `${resolution(file)} · ${formatSize(file.size)}`
}
