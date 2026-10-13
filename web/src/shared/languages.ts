// Nombres en castellano de los códigos de idioma que traen los videos (ISO 639-1 y 639-2).

const NAMES: Record<string, string> = {
  spa: 'español', es: 'español', esp: 'español',
  eng: 'inglés', en: 'inglés',
  jpn: 'japonés', ja: 'japonés', jp: 'japonés',
  por: 'portugués', pt: 'portugués',
  fre: 'francés', fra: 'francés', fr: 'francés',
  ger: 'alemán', deu: 'alemán', de: 'alemán',
  ita: 'italiano', it: 'italiano',
  kor: 'coreano', ko: 'coreano',
  chi: 'chino', zho: 'chino', zh: 'chino',
  rus: 'ruso', ru: 'ruso',
  ara: 'árabe', ar: 'árabe',
}

/** "jpn" → "japonés". Un código desconocido se muestra tal cual; "und" (sin definir), nada. */
export function languageName(code: string | null | undefined): string | null {
  if (!code || code === 'und') return null
  return NAMES[code.toLowerCase()] ?? code
}
