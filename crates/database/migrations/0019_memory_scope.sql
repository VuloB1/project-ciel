-- Alcance de cada memoria: quién puede "ver" lo que el personaje recuerda.
--   'private' = solo canales del dueño (Chat, Llamada)  [por defecto]
--   'public'  = también canales abiertos a terceros (Twitch, Discord)
-- Todo lo que ya existía queda 'private': una memoria sacada de una charla
-- privada nunca debe aparecer en un canal público por accidente — pasar a
-- 'public' es siempre una decisión explícita del usuario.
ALTER TABLE memories ADD COLUMN scope TEXT NOT NULL DEFAULT 'private';
