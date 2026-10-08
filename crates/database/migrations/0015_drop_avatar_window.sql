-- Revierte 0014_avatar_window.sql — la funcionalidad (glow/opacidad/tamaño
-- recordado de la ventana flotante) se descartó antes de terminar de
-- integrarse: el usuario prefiere no tocar esa área hasta rediseñar el
-- avatar por completo. Nunca se edita una migración ya aplicada, así que
-- esto se revierte con una nueva en vez de borrar/modificar 0014.
DROP TABLE profile_avatar_window;
