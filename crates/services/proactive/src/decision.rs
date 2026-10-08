use serde::Deserialize;

use aivt_database::Memory;

/// One-shot analysis call, not a creative response — kept low so the model
/// reliably returns parseable JSON instead of drifting into prose, same
/// reasoning as `aivt_memory::extraction::EXTRACTION_TEMPERATURE`.
pub const DECISION_TEMPERATURE: f32 = 0.4;
/// Generoso a propósito: un modelo local puede ser bastante florido
/// escribiendo el "message" (emojis, varias oraciones) — verificado en
/// vivo que 256 lo cortaba a mitad del JSON (`finish_reason: "length"`)
/// antes de cerrar la comilla/llave, dejando `parse_decision` sin nada
/// que interpretar.
pub const DECISION_MAX_TOKENS: u32 = 512;
/// How many memories to fold into the prompt — pinned first, then most
/// recent; enough for the model to notice a mentioned upcoming event
/// without turning the prompt into the whole memory list.
pub const MAX_MEMORIES_IN_PROMPT: usize = 8;

const SYSTEM_PROMPT: &str = "Sos quien decide si un personaje de IA le escribiría algo, ahora mismo, a la \
persona con la que suele hablar — sin que ella haya escrito nada. Te dan el contexto: cuánto hace que no \
hablan, el estado de ánimo actual del personaje, la fecha y hora de hoy, su tendencia a iniciar charla, y \
algunas de sus memorias recientes o importantes sobre esa persona.\n\n\
Decidí si corresponde escribir algo. Si el silencio es corto, no hay nada que decir, o no encaja con la \
personalidad, respondé que no. Si corresponde, redactá el mensaje ya en la voz del personaje (usando el \
contexto de personalidad y ánimo que se te da) — algo breve y natural, como lo mandaría una persona real, \
nunca un \"recordatorio\" formal ni algo genérico tipo \"¡hola! ¿cómo estás?\" sin motivo.\n\n\
Responde ÚNICAMENTE con un objeto JSON, sin texto antes o después, con este formato exacto:\n\
{\"should_initiate\": true o false, \"message\": \"...\" o null}\n\n\
Si decidís que no corresponde escribir nada, respondé exactamente: {\"should_initiate\": false, \"message\": null}";

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ProactiveDecision {
    #[serde(default)]
    pub should_initiate: bool,
    #[serde(default)]
    pub message: Option<String>,
}

impl ProactiveDecision {
    /// A model that says "yes" but leaves the message blank (or all
    /// whitespace) is treated as "no" — defensive against an inconsistent
    /// response rather than sending an empty character message.
    pub fn resolved_message(&self) -> Option<&str> {
        if !self.should_initiate {
            return None;
        }
        self.message.as_deref().map(str::trim).filter(|m| !m.is_empty())
    }
}

/// Everything the decision prompt needs — gathered by `ProactiveService`
/// from `MemoryManager`/`ProfilesRepository`/`ConversationsRepository`,
/// kept as plain data so `user_prompt` (and its tests) don't need any of
/// those dependencies themselves.
pub struct DecisionContext<'a> {
    pub character_context: String,
    pub hours_since_last_message: f64,
    pub now: chrono::DateTime<chrono::Utc>,
    pub initiate_conversation_tendency: i64,
    pub memories: &'a [Memory],
}

pub fn system_prompt() -> &'static str {
    SYSTEM_PROMPT
}

pub fn user_prompt(ctx: &DecisionContext<'_>) -> String {
    let mut prompt = String::new();
    prompt.push_str(&ctx.character_context);
    prompt.push_str("\n\n");
    prompt.push_str(&format!("Hace {:.1} horas que no hablan.\n", ctx.hours_since_last_message));
    prompt.push_str(&format!(
        "Hoy es {}, son las {}.\n",
        ctx.now.format("%A %d de %B de %Y"),
        ctx.now.format("%H:%M"),
    ));
    prompt.push_str(&format!("Tendencia a iniciar charla (0-100): {}.\n", ctx.initiate_conversation_tendency));

    if ctx.memories.is_empty() {
        prompt.push_str("No hay memorias registradas todavía.\n");
    } else {
        prompt.push_str("Memorias recientes o importantes:\n");
        for memory in ctx.memories.iter().take(MAX_MEMORIES_IN_PROMPT) {
            let body = memory.summary.as_deref().filter(|s| !s.trim().is_empty()).unwrap_or(&memory.content);
            prompt.push_str(&format!("- {}: {body}\n", memory.title));
        }
    }

    prompt
}

/// Mismo fallback que `aivt_memory::extraction::parse_extraction_response`
/// (JSON directo, luego sub-string delimitado por `{`/`}` para un modelo
/// que agrega prosa alrededor) — no se comparte código entre crates por lo
/// mismo que ese módulo ya documenta: lo bastante chico como para no
/// justificar el acoplamiento.
pub fn parse_decision(raw: &str) -> Option<ProactiveDecision> {
    let trimmed = raw.trim();

    if let Ok(decision) = serde_json::from_str::<ProactiveDecision>(trimmed) {
        return Some(decision);
    }
    let start = trimmed.find('{')?;
    let end = trimmed.rfind('}')?;
    if end < start {
        return None;
    }
    serde_json::from_str::<ProactiveDecision>(&trimmed[start..=end]).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_clean_yes_response() {
        let raw = r#"{"should_initiate": true, "message": "¿Cómo te fue hoy?"}"#;
        let decision = parse_decision(raw).unwrap();
        assert!(decision.should_initiate);
        assert_eq!(decision.resolved_message(), Some("¿Cómo te fue hoy?"));
    }

    #[test]
    fn parses_a_clean_no_response() {
        let raw = r#"{"should_initiate": false, "message": null}"#;
        let decision = parse_decision(raw).unwrap();
        assert!(!decision.should_initiate);
        assert_eq!(decision.resolved_message(), None);
    }

    #[test]
    fn extracts_the_object_even_with_surrounding_prose() {
        let raw = "Claro, aquí está mi decisión:\n{\"should_initiate\": true, \"message\": \"Hola!\"}\nEspero que ayude.";
        let decision = parse_decision(raw).unwrap();
        assert_eq!(decision.resolved_message(), Some("Hola!"));
    }

    #[test]
    fn missing_fields_default_to_not_initiating() {
        let decision = parse_decision("{}").unwrap();
        assert!(!decision.should_initiate);
        assert_eq!(decision.resolved_message(), None);
    }

    #[test]
    fn genuinely_unparseable_text_returns_none() {
        assert!(parse_decision("lo siento, no puedo ayudar con eso").is_none());
    }

    #[test]
    fn a_blank_message_is_treated_as_not_initiating_even_if_the_flag_says_yes() {
        let raw = r#"{"should_initiate": true, "message": "   "}"#;
        let decision = parse_decision(raw).unwrap();
        assert_eq!(decision.resolved_message(), None);
    }

    #[test]
    fn a_missing_message_with_should_initiate_true_resolves_to_none() {
        let raw = r#"{"should_initiate": true}"#;
        let decision = parse_decision(raw).unwrap();
        assert_eq!(decision.resolved_message(), None);
    }

    #[test]
    fn user_prompt_mentions_the_gathered_context() {
        let now = chrono::Utc::now();
        let ctx = DecisionContext {
            character_context: "Es cálida y curiosa.".to_string(),
            hours_since_last_message: 5.5,
            now,
            initiate_conversation_tendency: 70,
            memories: &[],
        };
        let prompt = user_prompt(&ctx);
        assert!(prompt.contains("Es cálida y curiosa."));
        assert!(prompt.contains("5.5 horas"));
        assert!(prompt.contains("70"));
        assert!(prompt.contains("No hay memorias registradas todavía."));
    }
}
