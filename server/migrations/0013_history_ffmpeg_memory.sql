-- Memoria de ffmpeg (la cola) en el historial, como ya está su CPU: bytes, el máximo del minuto.
ALTER TABLE server_samples ADD COLUMN memory_ffmpeg INTEGER NOT NULL DEFAULT 0;
