-- Perfiles: cada persona de la casa, dentro de la cuenta. Cada uno tiene su progreso.

CREATE TABLE profiles (
  id INTEGER PRIMARY KEY,
  name TEXT NOT NULL UNIQUE COLLATE NOCASE,
  created_at INTEGER NOT NULL
);

-- El primero existe siempre: lo visto hasta ahora pasa a ser suyo. Al crear la cuenta toma el
-- nombre que se elija.
INSERT INTO profiles (id, name, created_at) VALUES (1, 'Principal', strftime('%s', 'now'));

-- El progreso pasa a ser por perfil (SQLite no cambia una clave primaria: se rehace la tabla).
CREATE TABLE progress_new (
  profile_id INTEGER NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
  content_id INTEGER NOT NULL REFERENCES contents(id),
  position REAL NOT NULL,        -- segundos
  duration REAL,                 -- segundos, según el reproductor
  watched INTEGER NOT NULL DEFAULT 0,
  updated_at INTEGER NOT NULL,
  PRIMARY KEY (profile_id, content_id)
);
INSERT INTO progress_new (profile_id, content_id, position, duration, watched, updated_at)
  SELECT 1, content_id, position, duration, watched, updated_at FROM progress;
DROP TABLE progress;
ALTER TABLE progress_new RENAME TO progress;

-- El historial dice quién miró (con una copia del nombre, como el título).
ALTER TABLE playback_sessions ADD COLUMN profile_id INTEGER REFERENCES profiles(id) ON DELETE SET NULL;
ALTER TABLE playback_sessions ADD COLUMN profile_name TEXT;
UPDATE playback_sessions SET profile_id = 1, profile_name = 'Principal';
