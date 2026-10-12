// Piezas para navegar con el control remoto (o las flechas del teclado).

import { useEffect, type ReactNode } from 'react'
import { Link, useNavigate } from 'react-router'
import { FocusContext, useFocusable } from '@noriginmedia/norigin-spatial-navigation'

/**
 * Un link que se puede enfocar con las flechas. Enter lo abre, como un clic.
 * (La navegación espacial intercepta Enter, así que la apertura se hace acá).
 */
export function FocusableLink({
  to,
  state,
  className = '',
  focusKey,
  children,
}: {
  to: string
  state?: unknown
  className?: string
  focusKey?: string
  children: ReactNode
}) {
  const navigate = useNavigate()
  const { ref, focused, focusSelf } = useFocusable<object, HTMLAnchorElement>({
    focusKey,
    onEnterPress: () => navigate(to, { state }),
  })
  return (
    <Link
      ref={ref}
      to={to}
      state={state}
      className={`${className} ${focused ? 'is-focused' : ''}`}
      // Con el mouse, el foco sigue al puntero: así nunca hay dos elementos "elegidos".
      onMouseEnter={() => focusSelf()}
    >
      {children}
    </Link>
  )
}

/**
 * Un grupo de elementos enfocables (una fila de carátulas, una pantalla).
 * Recuerda cuál fue el último elegido: al volver a la fila, el foco vuelve ahí.
 */
export function FocusGroup({
  children,
  className,
  focusKey,
  preferredChildFocusKey,
  autoFocus = false,
}: {
  children: ReactNode
  className?: string
  focusKey?: string
  preferredChildFocusKey?: string
  autoFocus?: boolean
}) {
  const { ref, focusKey: key, focusSelf } = useFocusable<object, HTMLDivElement>({
    focusKey,
    preferredChildFocusKey,
    saveLastFocusedChild: true,
  })

  // Al abrir una pantalla, el foco arranca en su primer elemento (o el preferido).
  useEffect(() => {
    if (autoFocus) focusSelf()
  }, [autoFocus, focusSelf])

  return (
    <FocusContext.Provider value={key}>
      <div ref={ref} className={className}>
        {children}
      </div>
    </FocusContext.Provider>
  )
}
