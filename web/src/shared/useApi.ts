import { useCallback, useEffect, useState } from 'react'
import { api } from './api'

/**
 * Pide `path` a la API al montar el componente y cada vez que cambia. `reload()` vuelve a pedirlo.
 * Con `path` vacío no pide nada (para pedidos que dependen de una elección todavía no hecha).
 */
export function useApi<T>(path: string) {
  const [data, setData] = useState<T>()
  const [error, setError] = useState<string>()
  const [loading, setLoading] = useState(true)
  const [version, setVersion] = useState(0)

  useEffect(() => {
    if (!path) {
      setData(undefined)
      setLoading(false)
      return
    }
    let cancelled = false
    setLoading(true)
    api<T>(path)
      .then((result) => {
        if (cancelled) return
        setData(result)
        setError(undefined)
      })
      .catch((err: Error) => {
        if (!cancelled) setError(err.message)
      })
      .finally(() => {
        if (!cancelled) setLoading(false)
      })
    // Si el componente se desmonta o cambia `path`, se ignora la respuesta vieja.
    return () => {
      cancelled = true
    }
  }, [path, version])

  const reload = useCallback(() => setVersion((v) => v + 1), [])
  return { data, error, loading, reload }
}
