-- Historial: cada vez que alguien mira algo, desde que empieza hasta que deja de mirar.
-- Guarda una copia del título y de la biblioteca, así el historial se conserva aunque
-- el contenido se renombre o se borre.
CREATE TABLE playback_sessions (
  id INTEGER PRIMARY KEY,
  content_id INTEGER REFERENCES contents(id) ON DELETE SET NULL,
  library_id INTEGER REFERENCES libraries(id) ON DELETE SET NULL,
  title TEXT NOT NULL,                 -- "Sousou no Frieren · T1 E2", tal como se llamaba
  library_name TEXT,
  client TEXT NOT NULL,                -- IP del equipo que miró
  started_at INTEGER NOT NULL,         -- segundos desde 1970 (UTC)
  ended_at INTEGER NOT NULL,           -- se actualiza con cada aviso del reproductor
  seconds_watched REAL NOT NULL DEFAULT 0,
  start_position REAL NOT NULL,
  end_position REAL NOT NULL,
  duration REAL,
  completed INTEGER NOT NULL DEFAULT 0 -- pasó del 90 %
);

CREATE INDEX playback_sessions_by_start ON playback_sessions (started_at);
CREATE INDEX playback_sessions_by_content ON playback_sessions (content_id);
CREATE INDEX playback_sessions_by_library ON playback_sessions (library_id);
