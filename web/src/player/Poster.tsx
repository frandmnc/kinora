import { useState } from 'react'
import { mediaUrl } from '../shared/api'

/**
 * La URL de la carátula o el fondo de un contenido; `undefined` si no tiene. `at` (cuándo se
 * guardó) va en la URL: si la imagen cambia, el navegador no muestra la vieja.
 */
export function artUrl(contentId: number, kind: 'poster' | 'backdrop', at: number | null) {
  return at === null ? undefined : mediaUrl(`/api/contents/${contentId}/${kind}?v=${at}`)
}

/**
 * La carátula. Si no hay (o no carga), un degradé con la inicial: el color sale del título,
 * así cada contenido tiene siempre el mismo.
 */
export function Poster({ title, src }: { title: string; src?: string }) {
  const [failed, setFailed] = useState(false)
  if (src && !failed) {
    return <img className="poster poster-image" src={src} alt="" loading="lazy" onError={() => setFailed(true)} />
  }
  const hue = [...title].reduce((sum, char) => sum + char.charCodeAt(0), 0) % 360
  return (
    <div
      className="poster"
      style={{ background: `linear-gradient(160deg, hsl(${hue} 45% 32%), hsl(${(hue + 40) % 360} 50% 14%))` }}
    >
      <span>{title.charAt(0).toUpperCase()}</span>
    </div>
  )
}
