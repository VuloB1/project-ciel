-- Fijar conversaciones (mantenerlas siempre arriba) con orden manual entre
-- ellas. `pin_order` es un entero simple, no un timestamp: el pedido es
-- orden elegido a mano (arrastrar y soltar), no "fijado más reciente
-- primero" — se reescribe completo al reordenar, mismo criterio que otras
-- columnas libres del esquema (`conversations.source`, `memories.source`).
ALTER TABLE conversations ADD COLUMN is_pinned INTEGER NOT NULL DEFAULT 0;
ALTER TABLE conversations ADD COLUMN pin_order INTEGER NOT NULL DEFAULT 0;
