/** Para buscar sin que importen los acentos ni las mayúsculas: "Pokémon" encuentra "pokemon". */
function normalize(text: string) {
  // Las marcas combinables (U+0300 a U+036F): los acentos, la diéresis y la tilde de la ñ una
  // vez separados. `\p{M}` sería más general, pero es de Chrome 64 y la Samsung tiene el 63.
  return text.normalize('NFD').replace(/[̀-ͯ]/g, '').toLowerCase()
}

/** Lo que tiene en el título todas las palabras buscadas, en cualquier orden. */
export function searchTitles<T extends { title: string }>(items: T[], query: string) {
  const words = normalize(query).split(/\s+/).filter(Boolean)
  return items.filter((item) => words.every((word) => normalize(item.title).includes(word)))
}
