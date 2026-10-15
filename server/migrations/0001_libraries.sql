-- Bibliotecas: categorías ("Películas", "Series", "Anime"). Cada una es una subcarpeta
-- de `media_dir` (config.toml) que Kinora crea y renombra.
CREATE TABLE libraries (
  id INTEGER PRIMARY KEY,
  name TEXT NOT NULL UNIQUE,
  dir TEXT NOT NULL UNIQUE,      -- nombre de la carpeta dentro de media_dir
  icon TEXT                      -- ícono del menú ("clapperboard", "tv"); NULL: según lo que tiene
);
