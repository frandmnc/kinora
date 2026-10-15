// Los íconos que se pueden elegir para una biblioteca. Se guardan por nombre ("popcorn"); si no
// se eligió ninguno (o el nombre no está acá), se usa el automático.

import {
  BookOpen,
  Camera,
  Cat,
  Clapperboard,
  Drama,
  Film,
  Gamepad2,
  Ghost,
  Globe,
  GraduationCap,
  Heart,
  Laugh,
  LibraryBig,
  Mic,
  MonitorPlay,
  Music,
  Popcorn,
  Rocket,
  Skull,
  Smile,
  Sparkles,
  Star,
  Swords,
  Trophy,
  Tv,
  Video,
  type LucideIcon,
} from 'lucide-react'
import type { Content } from './types/Content'
import type { Library } from './types/Library'

export const LIBRARY_ICONS: { name: string; label: string; icon: LucideIcon }[] = [
  { name: 'clapperboard', label: 'Claqueta', icon: Clapperboard },
  { name: 'film', label: 'Película', icon: Film },
  { name: 'popcorn', label: 'Pochoclos', icon: Popcorn },
  { name: 'tv', label: 'Tele', icon: Tv },
  { name: 'monitor-play', label: 'Pantalla', icon: MonitorPlay },
  { name: 'video', label: 'Cámara de video', icon: Video },
  { name: 'sparkles', label: 'Destellos', icon: Sparkles },
  { name: 'swords', label: 'Espadas', icon: Swords },
  { name: 'drama', label: 'Máscaras', icon: Drama },
  { name: 'laugh', label: 'Risa', icon: Laugh },
  { name: 'heart', label: 'Corazón', icon: Heart },
  { name: 'ghost', label: 'Fantasma', icon: Ghost },
  { name: 'skull', label: 'Calavera', icon: Skull },
  { name: 'rocket', label: 'Cohete', icon: Rocket },
  { name: 'smile', label: 'Sonrisa', icon: Smile },
  { name: 'cat', label: 'Gato', icon: Cat },
  { name: 'globe', label: 'Mundo', icon: Globe },
  { name: 'graduation-cap', label: 'Birrete', icon: GraduationCap },
  { name: 'book-open', label: 'Libro', icon: BookOpen },
  { name: 'camera', label: 'Cámara', icon: Camera },
  { name: 'music', label: 'Música', icon: Music },
  { name: 'mic', label: 'Micrófono', icon: Mic },
  { name: 'trophy', label: 'Trofeo', icon: Trophy },
  { name: 'gamepad-2', label: 'Joystick', icon: Gamepad2 },
  { name: 'star', label: 'Estrella', icon: Star },
  { name: 'library-big', label: 'Estantería', icon: LibraryBig },
]

/**
 * El ícono de una biblioteca: el elegido, o si no, según lo que tiene (tele si hay más series,
 * claqueta si hay más películas; vacía o mientras carga, una estantería).
 */
export function libraryIcon(library: Library, contents: Content[] | undefined): LucideIcon {
  const chosen = LIBRARY_ICONS.find((option) => option.name === library.icon)
  if (chosen) return chosen.icon
  return automaticIcon(library, contents)
}

export function automaticIcon(library: Library, contents: Content[] | undefined): LucideIcon {
  const own = contents?.filter((c) => c.library_id === library.id) ?? []
  const shows = own.filter((c) => c.kind === 'show').length
  const movies = own.filter((c) => c.kind === 'movie').length
  if (shows + movies === 0) return LibraryBig
  return shows > movies ? Tv : Clapperboard
}
