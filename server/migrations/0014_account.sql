-- La cuenta de Kinora (una sola: la del dueño) y las sesiones abiertas.
CREATE TABLE account (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  username TEXT NOT NULL,
  password_hash TEXT NOT NULL,     -- Argon2id en formato PHC ($argon2id$v=19$...)
  created_at INTEGER NOT NULL
);

CREATE TABLE sessions (
  id INTEGER PRIMARY KEY,
  token_hash TEXT NOT NULL UNIQUE, -- SHA-256 del token: el token en sí solo lo tiene el navegador
  device TEXT,                     -- de qué navegador o equipo, para reconocerla
  client TEXT,                     -- IP desde donde se abrió
  created_at INTEGER NOT NULL,
  last_seen_at INTEGER NOT NULL    -- vence si no se usa en 180 días
);
