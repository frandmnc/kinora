-- Cola de importación: archivos que hay que procesar (remux, y más adelante conversión) antes de
-- entrar a la biblioteca. Mientras esperan, el archivo vive en <biblioteca>/.kinora-tmp/.
CREATE TABLE import_jobs (
  id INTEGER PRIMARY KEY,
  library_id INTEGER NOT NULL REFERENCES libraries(id),
  content_id INTEGER NOT NULL REFERENCES contents(id),
  source_name TEXT NOT NULL,     -- el nombre que tenía en la bandeja de entrada
  operation TEXT NOT NULL CHECK (operation IN ('remux', 'audio', 'video')),
  action TEXT NOT NULL CHECK (action IN ('added', 'replaced', 'added_version')),
  rel_path TEXT NOT NULL,        -- destino final, relativo a la carpeta de la biblioteca
  state TEXT NOT NULL CHECK (state IN ('pending', 'running', 'done', 'failed', 'cancelled')),
  progress REAL NOT NULL DEFAULT 0,  -- 0..1
  error TEXT,
  source_size INTEGER NOT NULL,
  result_size INTEGER,
  created_at INTEGER NOT NULL,   -- segundos desde 1970 (UTC)
  started_at INTEGER,
  finished_at INTEGER
);

CREATE INDEX import_jobs_by_state ON import_jobs (state, id);
