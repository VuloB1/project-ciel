//! Running summary + open threads of a conversation.
//!
//! The history window sent to the model is short (see `AIOrchestrator`), so what was
//! said before it would be forgotten. Every few messages the model is asked, in one
//! extra call, to fold the messages that left the window into a short summary and to
//! list the topics left pending. Both are stored on the conversation and shown to the
//! character as notes. Distinct from long-term memory: this answers "what were we
//! talking about", memory answers "what do I know about this person".

use std::sync::Arc;

use futures::StreamExt;
use serde::Deserialize;

use aivt_database::{Message, MessageRole, MessageStatus};
use aivt_provider_llm::{ChatMessage, ChatRequest, LLMProvider, MessageRole as LlmRole, ProviderError};

/// The newest messages are not summarized: they are still in the history window.
const KEEP_RECENT: i64 = 12;
/// New messages needed before the summary is refreshed (one extra model call each time).
const SUMMARY_STEP: i64 = 8;
const MAX_THREADS: usize = 5;
const MAX_SUMMARY_CHARS: usize = 1200;
const MAX_THREAD_CHARS: usize = 160;
const SUMMARY_TEMPERATURE: f32 = 0.2;
const SUMMARY_MAX_TOKENS: u32 = 700;

const SYSTEM_PROMPT: &str = "Eres quien lleva las notas de una conversación entre un usuario y un personaje de IA. \
Recibes las notas anteriores (si las hay) y un tramo nuevo de la conversación. Devuelve las notas actualizadas.\n\n\
- \"summary\": resumen de TODO lo hablado hasta ahora (notas anteriores + tramo nuevo) en máximo unas 120 palabras: \
de qué se habló, qué se decidió, qué datos se dijeron. Solo lo que se dijo de verdad en la conversación; no inventes \
ni supongas nada. Escríbelo en el mismo idioma que usa la conversación. Llama siempre \"el usuario\" a la \
persona que escribe y \"el personaje\" al personaje de IA, y deja claro quién dijo o hizo cada cosa: no atribuyas \
al personaje lo que contó el usuario ni al revés.\n\
- \"open_threads\": hasta 5 temas que quedaron pendientes y que el personaje podría retomar más adelante (algo que el \
usuario va a hacer, una promesa, una pregunta sin responder, algo que quedó esperando). Cada uno, una frase corta. \
Quita los que ya se resolvieron. Si no hay ninguno, una lista vacía.\n\n\
Responde ÚNICAMENTE con un objeto JSON, sin texto antes ni después:\n\
{\"summary\": \"...\", \"open_threads\": [\"...\"]}";

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SessionNotes {
    pub summary: String,
    #[serde(default)]
    pub open_threads: Vec<String>,
}

/// How many messages the summary should cover now, or `None` if it is not due yet.
pub fn due(total_messages: i64, summarized_through: i64) -> Option<i64> {
    let upto = total_messages - KEEP_RECENT;
    (upto - summarized_through >= SUMMARY_STEP).then_some(upto)
}

/// Parses the model's reply; tolerates prose around the JSON object. `None` when it is
/// unusable or the summary is empty, so a bad reply never overwrites good notes.
pub fn parse(raw: &str) -> Option<SessionNotes> {
    let trimmed = raw.trim();
    let candidate = match (trimmed.find('{'), trimmed.rfind('}')) {
        (Some(start), Some(end)) if end > start => &trimmed[start..=end],
        _ => return None,
    };
    let mut notes: SessionNotes = serde_json::from_str(candidate).ok()?;
    notes.summary = truncate(notes.summary.trim(), MAX_SUMMARY_CHARS);
    if notes.summary.is_empty() {
        return None;
    }
    notes.open_threads = notes
        .open_threads
        .iter()
        .map(|t| truncate(t.trim(), MAX_THREAD_CHARS))
        .filter(|t| !t.is_empty())
        .take(MAX_THREADS)
        .collect();
    Some(notes)
}

fn truncate(text: &str, max_chars: usize) -> String {
    text.chars().take(max_chars).collect()
}

fn transcript(messages: &[Message]) -> String {
    messages
        .iter()
        .filter(|m| m.status == MessageStatus::Complete && !m.content.trim().is_empty())
        .filter_map(|m| match m.role {
            MessageRole::User => Some(format!("Usuario: {}", m.content.trim())),
            MessageRole::Assistant => Some(format!("Personaje: {}", m.content.trim())),
            MessageRole::System => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Asks the model for the updated notes. `Ok(None)` when it answered but not usably.
pub async fn update(
    provider: Arc<dyn LLMProvider>,
    model: &str,
    previous: &SessionNotes,
    segment: &[Message],
) -> Result<Option<SessionNotes>, ProviderError> {
    let segment = transcript(segment);
    if segment.is_empty() {
        return Ok(None);
    }
    let mut input = String::new();
    if !previous.summary.trim().is_empty() {
        input.push_str(&format!("Notas anteriores:\n{}\n", previous.summary.trim()));
    }
    if !previous.open_threads.is_empty() {
        input.push_str("Temas pendientes anteriores:\n");
        for thread in &previous.open_threads {
            input.push_str(&format!("- {thread}\n"));
        }
    }
    input.push_str(&format!("\nTramo nuevo de la conversación:\n{segment}"));

    let request = ChatRequest {
        model: model.to_string(),
        messages: vec![ChatMessage::text(LlmRole::System, SYSTEM_PROMPT), ChatMessage::text(LlmRole::User, input)],
        temperature: Some(SUMMARY_TEMPERATURE),
        max_tokens: Some(SUMMARY_MAX_TOKENS),
        tools: Vec::new(),
        context: Vec::new(),
        contains_stored_memories: false,
    };
    let mut stream = provider.stream_chat(request);
    let mut raw = String::new();
    while let Some(chunk) = stream.next().await {
        raw.push_str(&chunk?.delta);
    }
    Ok(parse(&raw))
}

#[cfg(test)]
mod tests {
    use aivt_testing::MockLLMProvider;

    use super::*;

    fn message(role: MessageRole, content: &str) -> Message {
        Message {
            id: "m".to_string(),
            conversation_id: "c".to_string(),
            role,
            content: content.to_string(),
            status: MessageStatus::Complete,
            provider_id: None,
            model: None,
            input_tokens: None,
            output_tokens: None,
            created_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn the_summary_is_due_only_after_enough_messages_left_the_window() {
        assert_eq!(due(19, 0), None);
        assert_eq!(due(20, 0), Some(8));
        assert_eq!(due(27, 8), None);
        assert_eq!(due(28, 8), Some(16));
    }

    #[test]
    fn parse_accepts_clean_json_and_json_inside_prose() {
        let clean = parse(r#"{"summary": "Hablaron de un viaje.", "open_threads": ["Mañana tiene examen"]}"#).unwrap();
        assert_eq!(clean, SessionNotes { summary: "Hablaron de un viaje.".into(), open_threads: vec!["Mañana tiene examen".into()] });
        let wrapped = parse("Claro:\n```json\n{\"summary\": \"Algo.\"}\n```").unwrap();
        assert_eq!((wrapped.summary.as_str(), wrapped.open_threads.len()), ("Algo.", 0));
    }

    #[test]
    fn parse_rejects_unusable_replies_and_caps_the_lists() {
        assert!(parse("no sé").is_none());
        assert!(parse(r#"{"summary": "  ", "open_threads": []}"#).is_none());
        let many = format!(r#"{{"summary": "x", "open_threads": [{}]}}"#, (0..9).map(|i| format!("\"t{i}\"")).collect::<Vec<_>>().join(","));
        assert_eq!(parse(&many).unwrap().open_threads.len(), MAX_THREADS);
    }

    #[tokio::test]
    async fn update_sends_previous_notes_and_the_new_segment_and_returns_the_parsed_notes() {
        let mock = Arc::new(MockLLMProvider::with_response([r#"{"summary": "Resumen nuevo.", "open_threads": []}"#]));
        let previous = SessionNotes { summary: "Resumen viejo.".into(), open_threads: vec!["Examen mañana".into()] };
        let segment = vec![message(MessageRole::User, "me fue bien"), message(MessageRole::Assistant, "¡qué alegría!")];

        let notes = update(mock.clone(), "m", &previous, &segment).await.unwrap().unwrap();

        assert_eq!(notes.summary, "Resumen nuevo.");
        let sent = &mock.received_requests()[0].messages[1].content;
        assert!(sent.contains("Resumen viejo.") && sent.contains("- Examen mañana") && sent.contains("Usuario: me fue bien") && sent.contains("Personaje: ¡qué alegría!"));
    }

    /// Summarizes a REAL conversation in two steps (like the app does) and prints the notes:
    /// `AIVT_REAL_DB=aivt.db LAB_MODEL=... cargo test -p aivt-service-chat summarizes_a_real -- --ignored --nocapture`.
    #[tokio::test]
    #[ignore = "needs AIVT_REAL_DB and a model server (LAB_MODEL, optional LAB_BASE_URL)"]
    async fn summarizes_a_real_conversation() {
        use aivt_database::MessagesRepository;
        use aivt_provider_llm::OpenAiCompatibleProvider;

        let source = std::env::var("AIVT_REAL_DB").expect("set AIVT_REAL_DB");
        let model = std::env::var("LAB_MODEL").expect("set LAB_MODEL");
        let base_url = std::env::var("LAB_BASE_URL").unwrap_or_else(|_| "http://localhost:1234/v1".to_string());
        let copy = std::env::temp_dir().join("aivt-real-db-summary-test.db");
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{suffix}", copy.display()));
        }
        std::fs::copy(&source, &copy).unwrap();
        let pool = aivt_database::connect(&copy).await.unwrap();
        aivt_database::migrate(&pool).await.unwrap();

        let conversation_id: String = sqlx::query_scalar("SELECT conversation_id FROM messages GROUP BY conversation_id ORDER BY COUNT(*) DESC LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
        let messages = MessagesRepository::new(&pool).list_for_conversation(&conversation_id).await.unwrap();
        let provider: Arc<dyn LLMProvider> = Arc::new(OpenAiCompatibleProvider::new(base_url, None));

        let mut notes = SessionNotes { summary: String::new(), open_threads: vec![] };
        let mut through = 0;
        while let Some(upto) = due(messages.len() as i64, through) {
            let segment = &messages[through as usize..upto as usize];
            let started = std::time::Instant::now();
            let updated = update(provider.clone(), &model, &notes, segment).await.unwrap();
            println!("--- messages {through}..{upto} of {} ({:.1}s) ---", messages.len(), started.elapsed().as_secs_f32());
            match updated {
                Some(n) => {
                    println!("SUMMARY: {}
THREADS: {:?}", n.summary, n.open_threads);
                    notes = n;
                }
                None => println!("(unusable reply, notes kept)"),
            }
            through = upto;
        }
        assert!(!notes.summary.is_empty(), "the model never produced a usable summary");
        pool.close().await;
    }

    #[tokio::test]
    async fn an_empty_segment_makes_no_model_call() {
        let mock = Arc::new(MockLLMProvider::with_response(["{}"]));
        let result = update(mock.clone(), "m", &SessionNotes { summary: String::new(), open_threads: vec![] }, &[]).await.unwrap();
        assert!(result.is_none());
        assert!(mock.received_requests().is_empty());
    }
}
