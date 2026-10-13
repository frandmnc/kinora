-- De dónde salió cada subtítulo: el nombre del archivo que se subió. NULL = venía dentro del video.
ALTER TABLE subtitles ADD COLUMN source_name TEXT;
