-- Caché de emotes de 7TV/BTTV/FFZ por canal de Twitch (broadcaster_id) —
-- ver docs de la feature de emotes. `description` queda NULL hasta que el
-- usuario corre la descripción por visión a mano; un refresh de la lista
-- (`upsert_and_reconcile`) nunca la toca, así lo ya descripto sobrevive a
-- que el canal agregue/saque emotes.
CREATE TABLE twitch_emotes (
    broadcaster_id TEXT NOT NULL,
    source TEXT NOT NULL,
    emote_id TEXT NOT NULL,
    name TEXT NOT NULL,
    tags TEXT NULL,
    image_url TEXT NOT NULL,
    description TEXT NULL,
    fetched_at TEXT NOT NULL,
    described_at TEXT NULL,
    PRIMARY KEY (broadcaster_id, source, emote_id)
);
