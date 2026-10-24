# Cambios

Cada versión de Kinora, de la más nueva a la más vieja. El número sigue [SemVer](https://semver.org/lang/es/): el último sube con los arreglos, el del medio con lo nuevo; la 1.0 llega cuando Kinora se dé por estable.

Para publicar una versión: escribir acá su sección y correr `scripts/release.sh <versión>`.

## 0.2.0 (2026-10-24)

**Kinora se instala en Windows y en la Mac.**

- Windows: instalador con asistente en castellano (`Kinora-0.2.0-setup.exe`). Instala ffmpeg con winget si falta, abre el puerto para la tele en el firewall y deja Kinora en el menú Inicio. Al actualizar, cierra Kinora y guarda una copia de la base antes de reemplazarlo.
- Windows: Kinora vive en la bandeja, al lado del reloj, como Plex. Abre el Player y el Admin, copia la dirección para la tele, reinicia el servidor si se cae y lo apaga ordenado al salir.
- Mac: Kinora.app en la barra de menú, con lo mismo.
- El Player y el Admin van dentro del servidor: Kinora es un solo archivo, más ffmpeg. La configuración es opcional.

**Diseño nuevo, "Sala".**

- Negro cálido, Barlow y Barlow Condensed, una sola luz ámbar para el foco y el avance. Nada de copiar a Netflix ni a Airbnb.
- Inicio: lo último en grande, con la carátula, la sinopsis y lo que falta; lo demás empezado al costado.
- Admin de día en tonos bizcocho, de noche con la paleta del Player. Los recursos del Estado, en una franja.

**Para mirar.**

- Reproductor: volver a empezar y pasar al siguiente episodio. Toda la barra se maneja con el control remoto (Arriba entra a la barra).
- Cada episodio dice cuántas veces se vio, el tiempo total y cuándo fue la última; si no se terminó, cuánto se vio.
- Anda en la Samsung de 2019 (Tizen 5.0, Chromium 63): compilación, polyfills, la cookie de sesión y el CSS.
- Si un video no se puede reproducir, el reproductor dice por qué.

## 0.1.0 (2026-10-15)

Primera versión usada en la casa: bibliotecas, importación con análisis, cola de conversión a HEVC, metadata de TMDB, subtítulos (extraídos, subidos y de OpenSubtitles), perfiles, seguir viendo, historial y estadísticas, papelera y el Player para la tele con control remoto.
