-- Pestaña "Avatar" (Fase 7): "reactive PNG first" — un set fijo de tres
-- imágenes por personaje (idle/pensando/hablando), no un backend Live2D/VRM
-- todavía (ver crates/providers/avatar). Nullable: sin ellas, el overlay
-- sencillamente reutiliza el avatar de perfil estático como imagen única en
-- los tres estados, nunca queda roto por falta de configuración.
ALTER TABLE profiles ADD COLUMN avatar_idle_path TEXT;
ALTER TABLE profiles ADD COLUMN avatar_thinking_path TEXT;
ALTER TABLE profiles ADD COLUMN avatar_talking_path TEXT;
