//! `ProactiveService`: el motor que decide si el personaje activo inicia
//! una conversación por su cuenta — ver docs/ADR/007-proactive-mode.md.
//! Corre en segundo plano mientras la app está abierta, evaluando
//! periódicamente inactividad, mood, hora del día y memorias recientes en
//! una sola llamada de decisión al LLM.

mod decision;
mod service;

pub use service::ProactiveService;
