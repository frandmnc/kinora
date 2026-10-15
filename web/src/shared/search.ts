/** Para buscar sin que importen los acentos ni las mayúsculas: "Pokémon" encuentra "pokemon". */
function normalize(text: string) {
  return text.normalize('NFD').replace(/\p{M}/gu, '').toLowerCase()
}

/** Lo que tiene en el título todas las palabras buscadas, en cualquier orden. */
export function searchTitles<T extends { title: string }>(items: T[], query: string) {
  const words = normalize(query).split(/\s+/).filter(Boolean)
  return items.filter((item) => words.every((word) => normalize(item.title).includes(word)))
}
