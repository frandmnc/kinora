-- Historial del servidor: una fila por minuto con los promedios de las mediciones de ese minuto.
-- Se guardan los últimos 30 días.
CREATE TABLE server_samples (
  minute INTEGER PRIMARY KEY,      -- inicio del minuto, segundos desde 1970 (UTC)
  cpu_kinora REAL NOT NULL,        -- % de toda la CPU (0–100), sin contar ffmpeg
  cpu_ffmpeg REAL NOT NULL,        -- % de toda la CPU que usó la cola (0 si no había nada)
  cpu_system REAL NOT NULL,        -- % de toda la CPU, todo el equipo
  memory_kinora INTEGER NOT NULL,  -- bytes
  memory_used INTEGER NOT NULL,    -- bytes, todo el equipo
  memory_total INTEGER NOT NULL,
  bytes_sent INTEGER NOT NULL,     -- enviados durante ese minuto
  viewers INTEGER NOT NULL         -- máximo de personas mirando a la vez
);
