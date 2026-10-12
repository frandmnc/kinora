// Subida de archivos a la bandeja de entrada de Kinora.

import { mediaUrl } from './api'
import type { InboxFile } from './types/InboxFile'

const VIDEO_EXTENSIONS = ['mkv', 'mp4', 'm4v', 'mov', 'avi', 'webm', 'ts', 'm2ts']
const SUBTITLE_EXTENSIONS = ['srt', 'ass', 'ssa', 'vtt']

function extension(name: string) {
  return name.split('.').pop()?.toLowerCase() ?? ''
}

export function isVideo(name: string) {
  return VIDEO_EXTENSIONS.includes(extension(name)) && !name.startsWith('.')
}

export function isSubtitle(name: string) {
  return SUBTITLE_EXTENSIONS.includes(extension(name)) && !name.startsWith('.')
}

/**
 * Sube un archivo. Usa XMLHttpRequest en lugar de fetch porque es la única forma
 * de conocer el avance de una subida en el navegador.
 */
export function uploadFile(file: File, onProgress: (sent: number) => void): Promise<InboxFile> {
  return new Promise((resolve, reject) => {
    const request = new XMLHttpRequest()
    request.open('POST', mediaUrl(`/api/admin/uploads?name=${encodeURIComponent(file.name)}`))
    request.setRequestHeader('content-type', 'application/octet-stream')
    request.responseType = 'json'
    request.upload.onprogress = (event) => onProgress(event.loaded)
    request.onload = () => {
      if (request.status >= 200 && request.status < 300) resolve(request.response as InboxFile)
      else reject(new Error(request.response?.error ?? `error ${request.status}`))
    }
    request.onerror = () => reject(new Error('se cortó la conexión con el servidor'))
    request.send(file)
  })
}

/**
 * Los videos de lo que se soltó en la zona de importación. Si se soltó una carpeta,
 * la recorre entera (por ejemplo, "Frieren S01" con todos sus episodios).
 */
export async function videosFromDrop(dataTransfer: DataTransfer): Promise<File[]> {
  // Las entradas hay que pedirlas en el mismo momento del evento: después ya no están.
  const entries = [...dataTransfer.items]
    .map((item) => item.webkitGetAsEntry())
    .filter((entry): entry is FileSystemEntry => entry !== null)

  const files: File[] = []
  for (const entry of entries) await collect(entry, files)
  return files.filter((file) => isVideo(file.name)).sort((a, b) => a.name.localeCompare(b.name))
}

async function collect(entry: FileSystemEntry, files: File[]): Promise<void> {
  if (entry.isFile) {
    const file = await new Promise<File>((resolve, reject) => (entry as FileSystemFileEntry).file(resolve, reject))
    files.push(file)
  } else if (entry.isDirectory) {
    const reader = (entry as FileSystemDirectoryEntry).createReader()
    // readEntries devuelve las entradas de a tandas: hay que llamarlo hasta que no queden.
    for (;;) {
      const batch = await new Promise<FileSystemEntry[]>((resolve, reject) => reader.readEntries(resolve, reject))
      if (batch.length === 0) break
      for (const child of batch) await collect(child, files)
    }
  }
}
