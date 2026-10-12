-- Catálogo: qué es cada cosa. Las series y temporadas agrupan; películas y episodios se reproducen.
CREATE TABLE contents (
  id INTEGER PRIMARY KEY,
  library_id INTEGER NOT NULL REFERENCES libraries(id),
  kind TEXT NOT NULL CHECK (kind IN ('movie', 'show', 'season', 'episode')),
  parent_id INTEGER REFERENCES contents(id),
  number INTEGER,                -- número de temporada o de episodio
  title TEXT NOT NULL,
  year INTEGER,
  overview TEXT,
  dir TEXT,                      -- solo movie/show, relativo a la carpeta de la biblioteca
  created_at INTEGER NOT NULL,   -- segundos desde 1970 (UTC)
  UNIQUE (parent_id, number)
);

CREATE INDEX contents_by_library ON contents (library_id, kind);
CREATE INDEX contents_by_parent ON contents (parent_id);

-- Archivos que reproducen un contenido. Un contenido puede no tener archivo, o tener varios.
CREATE TABLE media_files (
  id INTEGER PRIMARY KEY,
  content_id INTEGER NOT NULL REFERENCES contents(id),
  rel_path TEXT NOT NULL UNIQUE, -- relativo a la carpeta de la biblioteca
  size INTEGER NOT NULL,
  duration REAL,
  video_codec TEXT,              -- ej. "hvc1.2.4.L120.B0"
  width INTEGER,
  height INTEGER,
  probe_json TEXT,               -- salida completa de ffprobe
  source_path TEXT,              -- de dónde se importó
  imported_at INTEGER NOT NULL
);

CREATE INDEX media_files_by_content ON media_files (content_id);
