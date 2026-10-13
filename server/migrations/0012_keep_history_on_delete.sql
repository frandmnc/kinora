-- Al eliminar un contenido, el historial queda: los minutos vistos por día y los trabajos de la
-- cola pierden la referencia (content_id = NULL) pero siguen contando en las métricas.
-- SQLite no puede cambiar una clave foránea: se rehacen las dos tablas y se copian los datos.

CREATE TABLE playback_daily_new (
  id INTEGER PRIMARY KEY,
  day TEXT NOT NULL,             -- AAAA-MM-DD, en la hora local del servidor
  content_id INTEGER REFERENCES contents(id) ON DELETE SET NULL,
  seconds REAL NOT NULL,
  UNIQUE (day, content_id)
);
INSERT INTO playback_daily_new (day, content_id, seconds)
  SELECT day, content_id, seconds FROM playback_daily;
DROP TABLE playback_daily;
ALTER TABLE playback_daily_new RENAME TO playback_daily;

CREATE TABLE import_jobs_new (
  id INTEGER PRIMARY KEY,
  library_id INTEGER NOT NULL REFERENCES libraries(id),
  content_id INTEGER REFERENCES contents(id) ON DELETE SET NULL,
  source_name TEXT NOT NULL,
  operation TEXT NOT NULL CHECK (operation IN ('remux', 'audio', 'video')),
  action TEXT NOT NULL CHECK (action IN ('added', 'replaced', 'added_version')),
  rel_path TEXT NOT NULL,
  state TEXT NOT NULL CHECK (state IN ('pending', 'running', 'done', 'failed', 'cancelled')),
  progress REAL NOT NULL DEFAULT 0,
  error TEXT,
  source_size INTEGER NOT NULL,
  result_size INTEGER,
  created_at INTEGER NOT NULL,
  started_at INTEGER,
  finished_at INTEGER,
  duration REAL
);
INSERT INTO import_jobs_new
  SELECT id, library_id, content_id, source_name, operation, action, rel_path, state, progress,
         error, source_size, result_size, created_at, started_at, finished_at, duration
  FROM import_jobs;
DROP TABLE import_jobs;
ALTER TABLE import_jobs_new RENAME TO import_jobs;
CREATE INDEX import_jobs_by_state ON import_jobs (state, id);
