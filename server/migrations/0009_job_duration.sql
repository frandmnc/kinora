-- Duración del video original, para calcular la velocidad de conversión (× tiempo real).
ALTER TABLE import_jobs ADD COLUMN duration REAL;
