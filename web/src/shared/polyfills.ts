// Lo que le falta al navegador de la Samsung de 2019 (Tizen 5.0, Chromium 63). La sintaxis
// nueva (`?.`, `??`…) la traduce Vite al compilar; estas funciones, no: se agregan acá, solo
// si no existen. Se importa antes que todo lo demás.

// Array.prototype.flat y flatMap: Chrome 69.
if (!Array.prototype.flat) {
  Object.defineProperty(Array.prototype, 'flat', {
    configurable: true,
    writable: true,
    value: function flat(this: unknown[], depth = 1): unknown[] {
      const result: unknown[] = []
      for (const item of this) {
        if (Array.isArray(item) && depth > 0) result.push(...item.flat(depth - 1))
        else result.push(item)
      }
      return result
    },
  })
}

if (!Array.prototype.flatMap) {
  Object.defineProperty(Array.prototype, 'flatMap', {
    configurable: true,
    writable: true,
    value: function flatMap(
      this: unknown[],
      callback: (value: unknown, index: number, array: unknown[]) => unknown,
      thisArg?: unknown,
    ): unknown[] {
      return this.map(callback, thisArg).flat(1)
    },
  })
}

// Object.fromEntries: Chrome 73.
if (!Object.fromEntries) {
  Object.defineProperty(Object, 'fromEntries', {
    configurable: true,
    writable: true,
    value: function fromEntries(entries: Iterable<readonly [PropertyKey, unknown]>) {
      const result: Record<PropertyKey, unknown> = {}
      for (const [key, value] of entries) result[key] = value
      return result
    },
  })
}

export {}
