use aivt_database::{Message, MessageRole, MessageStatus};

/// Cuántos mensajes (del usuario y del personaje, juntos) se recuerdan dentro de
/// una llamada. Más viejos que esto se olvidan: la llamada es una charla en
/// directo, y el contexto de varios minutos atrás encarece cada respuesta
/// (más tokens, más latencia) sin aportar casi nada.
pub const MAX_CALL_HISTORY_MESSAGES: usize = 20;

/// Cuántos mensajes (contando el nuevo) alimentan la búsqueda de recuerdos.
const RETRIEVAL_QUERY_WINDOW: usize = 4;

/// Lo que se dijo en la llamada en curso, en memoria. Antes cada turno llegaba al
/// modelo como si fuera el primero: el personaje no recordaba lo que se había
/// dicho diez segundos antes. No se guarda en la base de datos — vive lo que dura
/// la llamada (para conservarla está el botón de exportar al Chat).
#[derive(Debug, Default)]
pub struct CallHistory {
    turns: Vec<(MessageRole, String)>,
}

impl CallHistory {
    pub fn push(&mut self, role: MessageRole, text: &str) {
        let text = text.trim();
        if text.is_empty() {
            return;
        }
        self.turns.push((role, text.to_string()));
        let excess = self.turns.len().saturating_sub(MAX_CALL_HISTORY_MESSAGES);
        if excess > 0 {
            self.turns.drain(..excess);
        }
    }

    pub fn clear(&mut self) {
        self.turns.clear();
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.turns.len()
    }

    /// El texto con el que se buscan recuerdos relevantes: lo dicho en los últimos
    /// turnos más el mensaje nuevo, SIN la marca "[Mensaje por voz...]" (sus
    /// palabras —"mensaje", "voz", "llamada"— harían coincidir cualquier memoria
    /// que las mencione). Una frase vaga como "cuéntame más de eso" casi no
    /// comparte palabras con la memoria: el tema se nombró un turno o dos antes,
    /// por eso se mira una ventana corta y no solo el último mensaje (mismo
    /// criterio que `ChatService::retrieval_query`).
    pub fn retrieval_query_with(&self, new_user_text: &str) -> String {
        // Solo lo que dijo el usuario: las palabras del propio personaje (que pudo
        // inventar algo) no deben decidir qué recuerda.
        let mut parts: Vec<&str> =
            self.turns.iter().rev().filter(|(role, _)| *role == MessageRole::User).take(RETRIEVAL_QUERY_WINDOW - 1).map(|(_, text)| text.as_str()).collect();
        parts.reverse();
        parts.push(new_user_text.trim());
        parts.join(" ")
    }

    /// El historial previo como mensajes para el orquestador, más el mensaje
    /// nuevo del usuario al final (que todavía no se registró: solo se registra
    /// cuando el turno termina).
    pub fn as_messages_with(&self, new_user_text: &str) -> Vec<Message> {
        let mut messages: Vec<Message> = self.turns.iter().map(|(role, text)| call_message(*role, text)).collect();
        messages.push(call_message(MessageRole::User, new_user_text));
        messages
    }
}

/// Lo que dice el usuario por voz va marcado: el modelo debe saber que es una
/// llamada hablada (respuestas cortas, sin formato), no un mensaje escrito.
pub fn call_user_text(text: &str) -> String {
    format!("[Mensaje por voz en la Llamada] {text}")
}

fn call_message(role: MessageRole, text: &str) -> Message {
    Message {
        id: "call-turn".to_string(),
        conversation_id: "call".to_string(),
        role,
        content: if role == MessageRole::User && !text.starts_with("[Mensaje por voz") { call_user_text(text) } else { text.to_string() },
        status: MessageStatus::Complete,
        provider_id: None,
        model: None,
        input_tokens: None,
        output_tokens: None,
        created_at: chrono::Utc::now().to_rfc3339(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_turn_sees_everything_said_earlier_in_the_call_in_order() {
        let mut history = CallHistory::default();
        history.push(MessageRole::User, "me llamo Vulito");
        history.push(MessageRole::Assistant, "Mucho gusto, Vulito.");

        let messages = history.as_messages_with("¿cómo me llamo?");

        let roles: Vec<_> = messages.iter().map(|m| m.role).collect();
        assert_eq!(roles, vec![MessageRole::User, MessageRole::Assistant, MessageRole::User]);
        assert!(messages[0].content.contains("me llamo Vulito"));
        assert_eq!(messages[1].content, "Mucho gusto, Vulito.");
        assert!(messages[2].content.contains("¿cómo me llamo?"));
    }

    #[test]
    fn user_turns_are_marked_as_spoken_and_the_character_is_not() {
        let mut history = CallHistory::default();
        history.push(MessageRole::User, "hola");
        history.push(MessageRole::Assistant, "hola");
        let messages = history.as_messages_with("qué tal");
        assert!(messages[0].content.starts_with("[Mensaje por voz en la Llamada]"));
        assert_eq!(messages[1].content, "hola");
        assert!(messages[2].content.starts_with("[Mensaje por voz en la Llamada]"));
    }

    #[test]
    fn only_the_most_recent_messages_are_kept() {
        let mut history = CallHistory::default();
        for i in 0..(MAX_CALL_HISTORY_MESSAGES + 7) {
            history.push(MessageRole::User, &format!("mensaje {i}"));
        }
        assert_eq!(history.len(), MAX_CALL_HISTORY_MESSAGES);
        let messages = history.as_messages_with("nuevo");
        assert!(messages[0].content.contains("mensaje 7"), "se olvidan los más viejos, no los recientes");
    }

    #[test]
    fn the_retrieval_query_spans_recent_turns_without_the_voice_marker() {
        let mut history = CallHistory::default();
        history.push(MessageRole::User, "hoy fui a Bariloche");
        history.push(MessageRole::Assistant, "Qué bien, ¿cómo estuvo?");

        let query = history.retrieval_query_with("cuéntame más de eso");

        assert!(query.contains("Bariloche"), "el tema nombrado un turno antes sigue en la consulta");
        assert!(query.contains("cuéntame más de eso"));
        assert!(!query.contains("[Mensaje por voz"), "la marca de voz no debe contaminar la búsqueda");
    }

    #[test]
    fn the_retrieval_query_ignores_what_the_character_said() {
        let mut history = CallHistory::default();
        history.push(MessageRole::User, "¿cómo se llamaba mi perro?");
        history.push(MessageRole::Assistant, "Creo que se llamaba Toby");

        let query = history.retrieval_query_with("sí, ese");

        assert!(query.contains("perro") && !query.contains("Toby"), "query was: {query}");
    }

    #[test]
    fn the_retrieval_query_only_looks_at_a_short_window() {
        let mut history = CallHistory::default();
        for i in 0..10 {
            history.push(MessageRole::User, &format!("tema{i}"));
        }
        let query = history.retrieval_query_with("nuevo");
        assert!(query.contains("tema9") && query.contains("tema8") && query.contains("tema7"));
        assert!(!query.contains("tema6"), "los turnos más viejos que la ventana no cuentan");
        assert!(query.ends_with("nuevo"));
    }

    #[test]
    fn empty_text_is_not_recorded_and_clear_forgets_everything() {
        let mut history = CallHistory::default();
        history.push(MessageRole::User, "   ");
        assert_eq!(history.len(), 0);
        history.push(MessageRole::User, "hola");
        history.clear();
        assert_eq!(history.as_messages_with("x").len(), 1);
    }
}
