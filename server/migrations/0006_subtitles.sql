-- Subtítulos de un archivo, convertidos a WebVTT al importar y guardados al lado del video.
CREATE TABLE subtitles (
  id INTEGER PRIMARY KEY,
  media_file_id INTEGER NOT NULL REFERENCES media_files(id),
  rel_path TEXT NOT NULL UNIQUE,     -- el .vtt, relativo a la carpeta de la biblioteca
  original_rel_path TEXT,            -- el .ass original, si lo había (conserva los estilos)
  language TEXT,                     -- código ISO 639 tal como venía (spa, eng, es…)
  srclang TEXT,                      -- código de 2 letras para el navegador (es, en…)
  label TEXT NOT NULL,               -- "Español", "Inglés · Signs"…
  is_default INTEGER NOT NULL DEFAULT 0,
  forced INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX subtitles_by_file ON subtitles (media_file_id);
