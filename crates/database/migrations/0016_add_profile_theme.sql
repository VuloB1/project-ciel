-- Fase 11 Polish — preset temático completo por perfil (paleta + radios),
-- además del accentColor ya existente (ADR-008). NULL = sin preset,
-- "Personalizado": el comportamiento actual (solo accentColor) sin cambios.
ALTER TABLE profiles ADD COLUMN theme TEXT;
