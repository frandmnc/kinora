import { useEffect, useRef, useState } from 'react'
import { Check, Wand2 } from 'lucide-react'
import { automaticIcon, libraryIcon, LIBRARY_ICONS } from '../shared/libraryIcons'
import type { Content } from '../shared/types/Content'
import type { Library } from '../shared/types/Library'

/**
 * El ícono de una biblioteca (el que se ve en el menú del Player). Tocarlo abre la grilla para
 * elegir otro, o "Automático": según tenga más películas o series.
 */
export function IconPicker({
  library,
  contents,
  onChoose,
}: {
  library: Library
  contents: Content[] | undefined
  onChoose: (icon: string | null) => void
}) {
  // Dónde se abre la grilla (debajo del botón). Va con posición fija para que no la corte la tarjeta.
  const [open, setOpen] = useState<{ top: number; left: number }>()
  const ref = useRef<HTMLDivElement>(null)
  const Current = libraryIcon(library, contents)
  const Automatic = automaticIcon(library, contents)

  // Se cierra al tocar afuera, con Escape o al desplazar la página (quedaría lejos del botón).
  useEffect(() => {
    if (!open) return
    const close = () => setOpen(undefined)
    const onPointer = (event: PointerEvent) => {
      if (!ref.current?.contains(event.target as Node)) close()
    }
    const onKey = (event: KeyboardEvent) => {
      if (event.key === 'Escape') close()
    }
    document.addEventListener('pointerdown', onPointer)
    document.addEventListener('keydown', onKey)
    window.addEventListener('scroll', close, true)
    window.addEventListener('resize', close)
    return () => {
      document.removeEventListener('pointerdown', onPointer)
      document.removeEventListener('keydown', onKey)
      window.removeEventListener('scroll', close, true)
      window.removeEventListener('resize', close)
    }
  }, [open])

  function toggle(button: HTMLButtonElement) {
    if (open) return setOpen(undefined)
    const rect = button.getBoundingClientRect()
    setOpen({ top: rect.bottom + 6, left: rect.left })
  }

  function choose(icon: string | null) {
    setOpen(undefined)
    if (icon !== library.icon) onChoose(icon)
  }

  const chosen = LIBRARY_ICONS.some((option) => option.name === library.icon) ? library.icon : null

  return (
    <div className="icon-picker" ref={ref}>
      <button
        type="button"
        className="icon-picker-button"
        onClick={(e) => toggle(e.currentTarget)}
        title="Cambiar el ícono del menú del Player"
        aria-label={`Ícono de ${library.name}`}
        aria-expanded={open !== undefined}
      >
        <Current />
      </button>
      {open && (
        <div className="icon-picker-menu" role="listbox" aria-label="Íconos" style={open}>
          <button
            type="button"
            role="option"
            aria-selected={chosen === null}
            aria-label="Automático: tele si tiene más series, claqueta si tiene más películas"
            className={`icon-picker-auto ${chosen === null ? 'selected' : ''}`}
            onClick={() => choose(null)}
          >
            <Wand2 />
            <span className="grow">Automático</span>
            <Automatic className="icon-picker-preview" />
            {chosen === null && <Check className="icon-picker-check" />}
          </button>
          <div className="icon-picker-grid">
            {LIBRARY_ICONS.map(({ name, label, icon: Icon }) => (
              <button
                key={name}
                type="button"
                role="option"
                aria-selected={chosen === name}
                className={`icon-picker-option ${chosen === name ? 'selected' : ''}`}
                title={label}
                aria-label={label}
                onClick={() => choose(name)}
              >
                <Icon />
              </button>
            ))}
          </div>
        </div>
      )}
    </div>
  )
}
