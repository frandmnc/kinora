// Cliente de la API de Kinora, compartido por el Admin y el Player.

// Vacío = mismo origen que la página. La app de la TV va a configurar la dirección del servidor.
let serverUrl = ''

export function setServerUrl(url: string) {
  serverUrl = url.replace(/\/$/, '')
}

// El perfil elegido en el Player ("¿Quién está mirando?"). Va en cada pedido: el progreso,
// "Seguir viendo" y el historial son de cada perfil. El Admin no elige perfil.
let profileId: number | null = null

export function setProfile(id: number | null) {
  profileId = id
}

/** Evento que se dispara cuando el servidor contesta que no hay sesión. */
export const SIGNED_OUT = 'kinora:signed-out'

export class ApiError extends Error {
  status: number

  constructor(status: number, message: string) {
    super(message)
    this.status = status
  }
}

type Options = {
  method?: 'GET' | 'POST' | 'PUT' | 'PATCH' | 'DELETE'
  body?: unknown
  /** Un archivo como cuerpo, tal cual (en lugar de `body` en JSON). */
  file?: Blob
  /** El pedido se completa aunque la página se cierre (para el último aviso del reproductor). */
  keepalive?: boolean
}

export async function api<T>(path: string, options: Options = {}): Promise<T> {
  const hasBody = options.body !== undefined
  const headers: Record<string, string> = {}
  if (options.file) headers['content-type'] = 'application/octet-stream'
  else if (hasBody) headers['content-type'] = 'application/json'
  if (profileId !== null) headers['kinora-profile'] = String(profileId)
  const response = await fetch(serverUrl + path, {
    method: options.method ?? 'GET',
    headers,
    body: options.file ?? (hasBody ? JSON.stringify(options.body) : undefined),
    keepalive: options.keepalive,
    // La cookie de sesión. Hoy es lo que hace `fetch` por defecto en el mismo sitio, pero en
    // Chrome 71 y anteriores (la Samsung de 2019 tiene el 63) el defecto era no mandarla ni
    // guardarla: se entraba bien y la sesión se perdía en el acto.
    credentials: 'same-origin',
  })

  // Sin sesión (o venció): la app vuelve a la pantalla de entrada.
  if (response.status === 401 && !path.startsWith('/api/auth/')) {
    window.dispatchEvent(new Event(SIGNED_OUT))
  }

  if (!response.ok) {
    // Kinora responde {"error": "..."}; otros errores pueden venir como texto.
    const text = await response.text()
    let message = text || response.statusText
    try {
      message = JSON.parse(text).error ?? message
    } catch {
      // No era JSON: queda el texto tal cual.
    }
    throw new ApiError(response.status, message)
  }

  if (response.status === 204) return undefined as T
  return (await response.json()) as T
}

/** URL completa de un recurso del servidor, para usar en `<video src>` o `<img src>`. */
export function mediaUrl(path: string): string {
  return serverUrl + path
}

/**
 * La URL de la carátula, el fondo o el cuadro (de un episodio) de un contenido; `undefined` si no tiene. `at` (cuándo se
 * guardó) va en la URL: si la imagen cambia, el navegador no muestra la vieja.
 */
export function artUrl(contentId: number, kind: 'poster' | 'backdrop' | 'still', at: number | null) {
  return at === null ? undefined : mediaUrl(`/api/contents/${contentId}/${kind}?v=${at}`)
}
