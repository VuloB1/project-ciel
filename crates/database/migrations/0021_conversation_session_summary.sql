-- Resumen acumulativo de lo que ya salió de la ventana de historial de una
-- conversación, y los temas que quedaron pendientes. Los escribe el modelo
-- desde el chat; `summarized_through` cuenta cuántos mensajes ya entraron en
-- el resumen (mismo criterio que `memory_consolidated_through`).
-- `open_threads` es una lista JSON de textos cortos.
ALTER TABLE conversations ADD COLUMN session_summary TEXT NOT NULL DEFAULT '';
ALTER TABLE conversations ADD COLUMN open_threads TEXT NOT NULL DEFAULT '[]';
ALTER TABLE conversations ADD COLUMN summarized_through INTEGER NOT NULL DEFAULT 0;
