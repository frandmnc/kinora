import { useState } from 'react'

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
