<p align="center">
  <img src="docs/kinora.png" width="128" alt="Kinora">
</p>

<h1 align="center">Kinora</h1>

<p align="center">
  Películas y series de la casa, en la tele y en la compu.<br>
  Un servidor chico que reemplaza a Plex en la red local, sin cuentas en la nube ni transcodificar en vivo.
</p>

---

## Qué es

Kinora corre en la computadora que guarda los videos y entrega dos aplicaciones web desde un mismo puerto:

- **Kinora Player**, para mirar. Pensado para la tele a tres metros: se maneja con las flechas, Enter y Atrás del control, sin mouse.
- **Kinora Admin**, para ordenar. Desde la compu: bibliotecas, importación, cola de conversión, metadata, subtítulos y el estado del servidor.

Todo lo que entra queda en un único formato (MP4 con HEVC y AAC), así la reproducción siempre es directa: el servidor entrega el archivo tal cual y el equipo lo decodifica por hardware.

## Qué hace

**Para mirar**

- Perfiles para cada persona de la casa, con su propio progreso.
- "Seguir viendo": lo último en grande, con la carátula, la sinopsis y lo que falta, y lo demás empezado al costado.
- Bibliotecas con orden por título, recientes o año, y búsqueda que no distingue acentos ("pokemon" encuentra "Pokémon").
- Detalle de cada serie por temporada, con cuántas veces se vio cada episodio y cuándo fue la última.
- Reproductor con subtítulos, volver a empezar y pasar al siguiente episodio, manejable entero con el control remoto.

**Para ordenar**

- Importación: se suben los archivos, Kinora los analiza con ffprobe y decide si alcanza con cambiar el contenedor o hay que convertir.
- Cola de conversión a HEVC de 10 bits con prioridad baja, para que mirar algo vaya siempre primero. El HDR10 se conserva; Kinora nunca inventa HDR.
- Metadata, carátulas, fondos y cuadros de episodios desde TMDB.
- Subtítulos extraídos del original, subidos a mano o buscados en OpenSubtitles.
- Nombres de carpetas y archivos compatibles con Plex, para usar los dos en paralelo mientras dura la mudanza.
- Estado del servidor (CPU, memoria, envío de video), quién está mirando, historial y estadísticas de lo visto.
- Papelera: lo borrado va a una carpeta de cada biblioteca y se puede recuperar a mano hasta vaciarla.

## Cómo está hecho

| Parte | Tecnología |
|---|---|
| `server/` | Rust: [axum](https://github.com/tokio-rs/axum), SQLite con [sqlx](https://github.com/launchbadge/sqlx), ffmpeg y ffprobe del sistema |
| `web/` | React, TypeScript y Vite: dos aplicaciones (Player en `/`, Admin en `/admin/`) que comparten el cliente de la API |
| `web/src/shared/types/` | Tipos generados desde Rust con [ts-rs](https://github.com/Aleph-Alpha/ts-rs): no se editan a mano |
| `macos/` | Kinora.app: el servidor en la barra de menú de la Mac, como Plex |

En la compilación de producción, el Player y el Admin quedan dentro del binario del servidor: Kinora es un solo archivo, más ffmpeg.

El diseño completo (formato, importación, base de datos, autenticación, etapas) está en el [plan técnico](kinora-plan-tecnico.md).

## Instalar en la Mac

Hace falta [Rust](https://rustup.rs), [Node](https://nodejs.org) y ffmpeg (`brew install ffmpeg`).

```bash
macos/install.sh
```

Compila todo, arma Kinora.app, la copia a `/Applications` y la abre. Aparece un ícono en la barra de menú con lo necesario: abrir el Player y el Admin, copiar la dirección para la tele, reiniciar y ver los registros. Para actualizar, se corre de nuevo.

La primera vez, Kinora pide crear la cuenta (solo desde la compu o la red de la casa) y se elige la carpeta media en Admin → Configuración.

## Desarrollar

```bash
cd server && cargo run          # el servidor, en http://localhost:8090
cd web && npm run dev           # Vite en http://localhost:5173, con la API reenviada al servidor
cd server && cargo test         # los tests, y de paso regenera los tipos de TypeScript
cd web && npm run build         # typecheck, lint del CSS para las teles y compilación
```

La configuración es opcional: sin `config.toml`, Kinora escucha en el puerto 8090 de todas las interfaces y guarda la base en `~/.kinora`.

## Las teles

El Player tiene que andar en los navegadores de las teles de la casa, que son más viejos que un Chrome actual:

| Tele | Navegador |
|---|---|
| Samsung UN43RU7100 (2019, Tizen 5.0) | Chromium 63 |
| Hisense con VIDAA | Chromium 74 a 79 |

Por eso se compila para Chrome 63 y stylelint controla el CSS al compilar. Lo que stylelint no detecta está anotado al principio de `web/src/player/player.css`.

## Estado

En uso en la casa, todavía en desarrollo. Próximo: la versión para Windows, con instalador, y la app para la tele.
