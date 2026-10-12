-- Tiempo de reproducción por día y contenido, para las métricas.
CREATE TABLE playback_daily (
  day TEXT NOT NULL,             -- AAAA-MM-DD, en la hora local del servidor
  content_id INTEGER NOT NULL REFERENCES contents(id),
  seconds REAL NOT NULL,
  PRIMARY KEY (day, content_id)
);
