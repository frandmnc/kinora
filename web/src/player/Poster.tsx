/**
 * Carátula provisoria: un degradé con la inicial. El color sale del título,
 * así cada contenido tiene siempre el mismo. Las imágenes reales llegan más adelante.
 */
export function Poster({ title }: { title: string }) {
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
