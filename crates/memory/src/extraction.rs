use serde::Deserialize;

use aivt_database::Message;

const VALID_CATEGORIES: [&str; 5] = ["person", "preference", "event", "fact", "other"];
const MAX_TITLE_LENGTH: usize = 48;
/// Soft cap communicated to the model for `content` — "denso, no pesado"
/// (pedido del usuario): detailed enough to be useful on its own, not an
/// unbounded essay that bloats storage or (if ever folded into a prompt)
/// token usage.
const MAX_CONTENT_CHARS_HINT: usize = 500;

const EXTRACTION_SYSTEM_PROMPT_TEMPLATE: &str = "Eres un extractor de memoria a largo plazo para un personaje de IA. \
Analiza la conversación que te dan y extrae solo datos nuevos y duraderos que valga la pena recordar en \
conversaciones futuras: preferencias del usuario, hechos concretos, eventos importantes, personas que \
menciona. Ignora saludos, charla trivial, o cosas ya evidentes por el contexto del personaje.\n\n\
Para cada memoria da tres partes con propósitos distintos — nunca dejes ninguna vacía:\n\
- \"title\": título muy breve (pocas palabras) que identifique la memoria de un vistazo, como el título de \
una nota.\n\
- \"summary\": un resumen compacto (una frase) pero AUTOCONTENIDO — debe poder entenderse por sí solo, sin \
haber leído la conversación original, porque es exactamente lo que se le recuerda al personaje en cada \
conversación futura. No escribas algo vago como \"habló de sus gustos\"; escribe el dato en sí, p. ej. \
\"Le gusta el senderismo de montaña.\"\n\
- \"content\": la versión más detallada y extensa, útil para consultar el contexto completo más adelante. \
Suficientemente detallada como para ser realmente útil, pero sin convertirse en un ensayo — máximo \
aproximadamente {max_content_chars} caracteres.\n\n\
Además, identifica por separado cualquier persona real mencionada por su nombre propio — nunca personajes \
ficticios, marcas, ni el propio personaje de IA — que valga la pena que el personaje reconozca la próxima \
vez que la mencionen; esto ayuda a que el reconocimiento de voz entienda mejor los nombres reales al \
hablar. Para cada persona:\n\
- \"name\": el nombre tal como se menciona.\n\
- \"alias\": un apodo o forma alternativa del mismo nombre si se menciona uno; deja \"\" si no aplica.\n\
- \"role\": \"self\" solo cuando el propio usuario se presenta o dice cómo se llama o quiere que le \
llamen; \"nickname\" para un apodo adicional del usuario; \"friend\" para cualquier otra persona real \
(amigos, familiares, conocidos).\n\
- \"confidence\": 0-100.\n\n\
Además, evaluá si el tramo de conversación debería cambiar el estado de ánimo del personaje — no su \
personalidad, sino cómo se siente en este momento. Da un objeto \"mood\" con:\n\
- \"valence_delta\": -15 a 15. Positivo si el tramo fue agradable/alegre para el personaje, negativo si fue \
desagradable/triste/tenso, 0 si no hubo ningún cambio de ánimo notable.\n\
- \"energy_delta\": -15 a 15. Positivo si el tramo lo dejó más animado/enérgico, negativo si lo dejó más \
apagado/cansado, 0 si no hubo cambio.\n\
No hace falta forzar un cambio en cada respuesta — la mayoría de los tramos de charla normal no deberían \
mover el ánimo en absoluto (0, 0). Reservalo para momentos genuinamente marcados.\n\n\
Ejemplo — para la frase \"me encanta el senderismo, sobre todo en la montaña, este fin de semana subí el \
Cerro Torre con mi hermano Kenji\", una respuesta bien formada sería:\n\
{\"memories\": [{\"title\": \"Afición al senderismo\", \"summary\": \"Le encanta el senderismo de montaña.\", \
\"content\": \"Le encanta el senderismo, especialmente en la montaña. Recientemente subió el Cerro Torre \
acompañado de su hermano.\", \"category\": \"preference\", \"importance\": 60, \"confidence\": 90}], \
\"people\": [{\"name\": \"Kenji\", \"alias\": \"\", \"role\": \"friend\", \"confidence\": 80}], \
\"mood\": {\"valence_delta\": 5, \"energy_delta\": 5}}\n\n\
Responde ÚNICAMENTE con un objeto JSON, sin ningún texto antes o después, con este formato exacto:\n\
{\"memories\": [{\"title\": \"...\", \"summary\": \"...\", \"content\": \"...\", \"category\": \"person|preference|event|fact|other\", \"importance\": 0-100, \"confidence\": 0-100}], \
\"people\": [{\"name\": \"...\", \"alias\": \"...\", \"role\": \"self|nickname|friend\", \"confidence\": 0-100}], \
\"mood\": {\"valence_delta\": -15 a 15, \"energy_delta\": -15 a 15}}\n\n\
Si no hay nada que valga la pena registrar y el ánimo no cambió, respondé exactamente: \
{\"memories\": [], \"people\": [], \"mood\": {\"valence_delta\": 0, \"energy_delta\": 0}}";

#[derive(Debug, Clone, Deserialize)]
pub struct ExtractedCandidate {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
    pub content: String,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub importance: Option<i64>,
    #[serde(default)]
    pub confidence: Option<i64>,
}

impl ExtractedCandidate {
    /// The model was asked for a title, but local models don't always
    /// comply — fall back to a truncated `content` rather than leaving the
    /// column empty (it's `NOT NULL` in the schema on purpose: a blank
    /// title defeats the whole point of the redesign, a scannable list).
    pub fn resolved_title(&self) -> String {
        match self.title.as_deref().map(str::trim) {
            Some(t) if !t.is_empty() => truncate_at_word_boundary(t, MAX_TITLE_LENGTH),
            _ => truncate_at_word_boundary(&self.content, MAX_TITLE_LENGTH),
        }
    }

    /// `None` when the model didn't provide a real summary — the caller
    /// (`AIOrchestrator`'s memory section) already falls back to `content`
    /// in that case, so there's no need to duplicate that fallback here.
    pub fn resolved_summary(&self) -> Option<String> {
        self.summary.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(str::to_string)
    }

    pub fn normalized_category(&self) -> String {
        match self.category.as_deref() {
            Some(c) if VALID_CATEGORIES.contains(&c) => c.to_string(),
            _ => "other".to_string(),
        }
    }

    pub fn clamped_importance(&self) -> i64 {
        self.importance.unwrap_or(50).clamp(0, 100)
    }

    pub fn clamped_confidence(&self) -> i64 {
        self.confidence.unwrap_or(50).clamp(0, 100)
    }
}

/// A person mentioned by name in the conversation — feeds
/// `aivt_database::PeopleRepository::upsert_by_name`, which in
/// turn feeds `CallService::transcription_prompt`'s Whisper priming (see
/// `crates/memory/src/people.rs`). Deliberately much thinner than
/// `ExtractedCandidate`: this only ever needs to become a short list of
/// names, never prose.
#[derive(Debug, Clone, Deserialize)]
pub struct ExtractedPersonCandidate {
    pub name: String,
    #[serde(default)]
    pub alias: Option<String>,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub confidence: Option<i64>,
}

impl ExtractedPersonCandidate {
    /// Defaults to `"friend"` — the same "call it a fact, not a promise"
    /// stance as `ExtractedCandidate::normalized_category`'s fallback to
    /// `"other"`: an unrecognized/missing role shouldn't be dropped, and
    /// "friend" is the least presumptuous guess (unlike "self", which
    /// would wrongly claim the AI knows this is the user's own name).
    pub fn normalized_role(&self) -> &'static str {
        match self.role.as_deref() {
            Some("self") => "self",
            Some("nickname") => "nickname",
            _ => "friend",
        }
    }

    pub fn clamped_confidence(&self) -> i64 {
        self.confidence.unwrap_or(50).clamp(0, 100)
    }

    pub fn trimmed_alias(&self) -> Option<&str> {
        self.alias.as_deref().map(str::trim).filter(|a| !a.is_empty())
    }
}

/// How the character's *momentary* emotional state (distinct from its
/// static `ProfilePersonality`) should shift after this stretch of
/// conversation — see `docs/ADR/006-mood.md`. Deltas, not absolute
/// values: the model judges "how much did this shift things", not a
/// (likely wrong) absolute mood it would have to invent.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct MoodAdjustmentCandidate {
    #[serde(default)]
    pub valence_delta: Option<i64>,
    #[serde(default)]
    pub energy_delta: Option<i64>,
}

impl MoodAdjustmentCandidate {
    pub fn clamped_valence_delta(&self) -> i64 {
        self.valence_delta.unwrap_or(0).clamp(-15, 15)
    }

    pub fn clamped_energy_delta(&self) -> i64 {
        self.energy_delta.unwrap_or(0).clamp(-15, 15)
    }
}

/// The model's full response: memories, people, and a mood adjustment
/// extracted from the same conversation in one call, rather than
/// separate round trips.
///
/// `deny_unknown_fields` is deliberate, not incidental: every field is
/// optional (`#[serde(default)]`), so *without* it, a stray JSON object
/// with none of these keys — e.g. a single bare `ExtractedCandidate` from
/// the *old* pre-`people` response shape, caught by `parse_extraction_response`'s
/// `{`...`}`-substring fallback — would "successfully" parse into an
/// all-empty `ExtractionResponse`, silently discarding real data instead
/// of falling through to the old-shape fallback that actually understands
/// it (a real bug caught by `falls_back_to_the_old_bare_array_shape`
/// during development). Denying unknown fields makes that mismatch a
/// parse failure instead, so the fallback chain behaves as intended.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ExtractionResponse {
    #[serde(default)]
    pub memories: Vec<ExtractedCandidate>,
    #[serde(default)]
    pub people: Vec<ExtractedPersonCandidate>,
    /// `None` both when the model omits the field entirely (a weak model
    /// ignoring the instruction) and when it explicitly sends a
    /// (0, 0) no-op — the caller treats both identically, applying only
    /// time-based decay.
    #[serde(default)]
    pub mood: Option<MoodAdjustmentCandidate>,
}

/// Same word-boundary-then-ellipsis approach as
/// `aivt_service_chat::title::title_from_first_message` — small enough,
/// and specific enough to each caller's default text, that sharing it
/// across crates isn't worth the coupling.
fn truncate_at_word_boundary(text: &str, max_len: usize) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return "Memoria".to_string();
    }
    if trimmed.chars().count() <= max_len {
        return trimmed.to_string();
    }

    let mut truncated: String = trimmed.chars().take(max_len).collect();
    if let Some(last_space) = truncated.rfind(' ') {
        if last_space > max_len / 2 {
            truncated.truncate(last_space);
        }
    }
    format!("{}…", truncated.trim_end())
}

pub fn system_prompt() -> String {
    EXTRACTION_SYSTEM_PROMPT_TEMPLATE.replace("{max_content_chars}", &MAX_CONTENT_CHARS_HINT.to_string())
}

/// Renders `messages` as a plain-text transcript for the extraction prompt
/// — deliberately not reusing `PromptComposer` (that composes the
/// *character's* system prompt; this is a completely different, one-shot
/// analysis request with its own instructions).
pub fn transcript(messages: &[Message]) -> String {
    messages
        .iter()
        .map(|m| {
            let speaker = match m.role {
                aivt_database::MessageRole::User => "Usuario",
                aivt_database::MessageRole::Assistant => "Personaje",
                aivt_database::MessageRole::System => "Sistema",
            };
            format!("{speaker}: {}", m.content)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Parses the model's raw text response into `{memories, people}`. Local
/// models don't always follow instructions perfectly, so this tries, in
/// order: the new object shape directly; an object substring within
/// surrounding prose; the *old* bare-array shape (from before `people`
/// existed) directly, treated as `memories` with no `people`; and an array
/// substring within surrounding prose, same treatment — a weak model that
/// ignores the new instruction and reverts to its old habit still yields
/// memories instead of nothing. Still returns `None` (not an error) for a
/// genuinely empty response or one that matches none of these shapes.
pub fn parse_extraction_response(raw: &str) -> Option<ExtractionResponse> {
    let trimmed = raw.trim();

    if let Ok(response) = serde_json::from_str::<ExtractionResponse>(trimmed) {
        return Some(response);
    }
    if let Some(response) = extract_delimited(trimmed, '{', '}').and_then(|s| serde_json::from_str::<ExtractionResponse>(s).ok()) {
        return Some(response);
    }
    if let Ok(memories) = serde_json::from_str::<Vec<ExtractedCandidate>>(trimmed) {
        return Some(ExtractionResponse { memories, ..Default::default() });
    }
    if let Some(memories) = extract_delimited(trimmed, '[', ']').and_then(|s| serde_json::from_str::<Vec<ExtractedCandidate>>(s).ok()) {
        return Some(ExtractionResponse { memories, ..Default::default() });
    }
    None
}

fn extract_delimited(text: &str, open: char, close: char) -> Option<&str> {
    let start = text.find(open)?;
    let end = text.rfind(close)?;
    if end < start {
        return None;
    }
    Some(&text[start..=end])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_clean_object_response() {
        let raw = r#"{"memories": [{"content": "Le gusta el café", "category": "preference", "importance": 60, "confidence": 90}], "people": [{"name": "Kenji", "role": "friend", "confidence": 80}]}"#;
        let response = parse_extraction_response(raw).unwrap();
        assert_eq!(response.memories.len(), 1);
        assert_eq!(response.memories[0].content, "Le gusta el café");
        assert_eq!(response.memories[0].normalized_category(), "preference");
        assert_eq!(response.people.len(), 1);
        assert_eq!(response.people[0].name, "Kenji");
        assert_eq!(response.people[0].normalized_role(), "friend");
    }

    #[test]
    fn an_empty_object_parses_to_nothing() {
        let response = parse_extraction_response(r#"{"memories": [], "people": []}"#).unwrap();
        assert!(response.memories.is_empty());
        assert!(response.people.is_empty());
    }

    #[test]
    fn extracts_the_object_even_with_surrounding_prose() {
        let raw = "Claro, aquí está el análisis:\n{\"memories\": [{\"content\": \"algo\"}], \"people\": []}\nEspero que ayude.";
        let response = parse_extraction_response(raw).unwrap();
        assert_eq!(response.memories.len(), 1);
        assert_eq!(response.memories[0].content, "algo");
    }

    #[test]
    fn falls_back_to_the_old_bare_array_shape() {
        // A weak local model that ignores the new object-shape instruction
        // and reverts to the pre-`people` habit still yields memories.
        let raw = r#"[{"content": "algo", "category": "fact"}]"#;
        let response = parse_extraction_response(raw).unwrap();
        assert_eq!(response.memories.len(), 1);
        assert!(response.people.is_empty());
    }

    #[test]
    fn genuinely_unparseable_text_returns_none() {
        assert!(parse_extraction_response("lo siento, no puedo ayudar con eso").is_none());
    }

    #[test]
    fn missing_optional_fields_fall_back_to_sane_defaults() {
        let raw = r#"{"memories": [{"content": "algo"}], "people": [{"name": "Ana"}]}"#;
        let response = parse_extraction_response(raw).unwrap();
        assert_eq!(response.memories[0].normalized_category(), "other");
        assert_eq!(response.memories[0].clamped_importance(), 50);
        assert_eq!(response.memories[0].clamped_confidence(), 50);
        assert_eq!(response.people[0].normalized_role(), "friend");
        assert_eq!(response.people[0].clamped_confidence(), 50);
        assert_eq!(response.people[0].trimmed_alias(), None);
    }

    #[test]
    fn an_unknown_category_falls_back_to_other() {
        let raw = r#"{"memories": [{"content": "algo", "category": "chisme"}], "people": []}"#;
        let response = parse_extraction_response(raw).unwrap();
        assert_eq!(response.memories[0].normalized_category(), "other");
    }

    #[test]
    fn an_unknown_role_falls_back_to_friend_not_self() {
        let raw = r#"{"memories": [], "people": [{"name": "Ana", "role": "boss"}]}"#;
        let response = parse_extraction_response(raw).unwrap();
        assert_eq!(response.people[0].normalized_role(), "friend");
    }

    #[test]
    fn out_of_range_scores_are_clamped() {
        let raw = r#"{"memories": [{"content": "algo", "importance": 500, "confidence": -20}], "people": [{"name": "Ana", "confidence": 999}]}"#;
        let response = parse_extraction_response(raw).unwrap();
        assert_eq!(response.memories[0].clamped_importance(), 100);
        assert_eq!(response.memories[0].clamped_confidence(), 0);
        assert_eq!(response.people[0].clamped_confidence(), 100);
    }

    #[test]
    fn resolved_title_uses_the_models_title_when_present() {
        let raw = r#"{"memories": [{"title": "Bebida favorita", "content": "Le encanta el café por las mañanas"}], "people": []}"#;
        let response = parse_extraction_response(raw).unwrap();
        assert_eq!(response.memories[0].resolved_title(), "Bebida favorita");
    }

    #[test]
    fn resolved_title_falls_back_to_a_truncated_content_when_missing() {
        let raw = r#"{"memories": [{"content": "Le encanta el café por las mañanas antes de trabajar todos los días de la semana"}], "people": []}"#;
        let response = parse_extraction_response(raw).unwrap();
        let title = response.memories[0].resolved_title();
        assert!(title.ends_with('…'));
        assert!(title.chars().count() <= 49);
    }

    #[test]
    fn resolved_summary_is_none_when_the_model_omits_it() {
        let raw = r#"{"memories": [{"content": "algo"}], "people": []}"#;
        let response = parse_extraction_response(raw).unwrap();
        assert_eq!(response.memories[0].resolved_summary(), None);
    }

    #[test]
    fn trimmed_alias_ignores_blank_strings() {
        let raw = r#"{"memories": [], "people": [{"name": "Ana", "alias": "   "}]}"#;
        let response = parse_extraction_response(raw).unwrap();
        assert_eq!(response.people[0].trimmed_alias(), None);
    }

    #[test]
    fn parses_a_mood_adjustment_when_present() {
        let raw = r#"{"memories": [], "people": [], "mood": {"valence_delta": 8, "energy_delta": -3}}"#;
        let response = parse_extraction_response(raw).unwrap();
        let mood = response.mood.unwrap();
        assert_eq!(mood.clamped_valence_delta(), 8);
        assert_eq!(mood.clamped_energy_delta(), -3);
    }

    #[test]
    fn mood_is_none_when_the_model_omits_it() {
        let raw = r#"{"memories": [], "people": []}"#;
        let response = parse_extraction_response(raw).unwrap();
        assert!(response.mood.is_none());
    }

    #[test]
    fn mood_deltas_out_of_range_are_clamped() {
        let raw = r#"{"memories": [], "people": [], "mood": {"valence_delta": 500, "energy_delta": -500}}"#;
        let response = parse_extraction_response(raw).unwrap();
        let mood = response.mood.unwrap();
        assert_eq!(mood.clamped_valence_delta(), 15);
        assert_eq!(mood.clamped_energy_delta(), -15);
    }

    #[test]
    fn a_missing_mood_delta_field_defaults_to_zero() {
        let raw = r#"{"memories": [], "people": [], "mood": {}}"#;
        let response = parse_extraction_response(raw).unwrap();
        let mood = response.mood.unwrap();
        assert_eq!(mood.clamped_valence_delta(), 0);
        assert_eq!(mood.clamped_energy_delta(), 0);
    }

    #[test]
    fn system_prompt_mentions_mood() {
        let prompt = system_prompt();
        assert!(prompt.contains("valence_delta"));
        assert!(prompt.contains("energy_delta"));
    }

    #[test]
    fn system_prompt_mentions_the_new_object_shape() {
        let prompt = system_prompt();
        assert!(prompt.contains("memories"));
        assert!(prompt.contains("people"));
        assert!(prompt.contains("title"));
        assert!(prompt.contains("summary"));
        assert!(prompt.contains("content"));
    }
}
