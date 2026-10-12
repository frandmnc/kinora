CREATE TABLE libraries (
  id INTEGER PRIMARY KEY,
  name TEXT NOT NULL UNIQUE,      -- "Películas", "Series", "Anime"
  root_dir TEXT NOT NULL UNIQUE   -- carpeta destino, ruta absoluta
);
