-- Progreso de reproducción: dónde quedó cada película o episodio.
CREATE TABLE progress (
  content_id INTEGER PRIMARY KEY REFERENCES contents(id),
  position REAL NOT NULL,        -- segundos
  duration REAL,                 -- segundos, según el reproductor
  watched INTEGER NOT NULL DEFAULT 0,
  updated_at INTEGER NOT NULL
);
