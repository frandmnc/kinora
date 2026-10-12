-- Configuración que se edita desde el Admin (por ejemplo, la carpeta media).
-- La de config.toml es la que hace falta antes de abrir la base: puerto, carpeta de datos.
CREATE TABLE settings (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
