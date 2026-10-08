use serde::{Deserialize, Serialize};

use aivt_database::Memory;

/// One proposed merge, ready to show the user before anything is written —
/// `originals` are the *full* `Memory` rows (not just ids) so the frontend
/// can render a real preview ("these become this") without a second
/// round-trip. Produced by `MemoryManager::propose_fusion`, never touches
/// the database on its own.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FusionProposal {
    pub originals: Vec<Memory>,
    pub title: String,
    pub summary: Option<String>,
    pub content: String,
    pub category: String,
    pub importance: i64,
}

/// What the frontend sends back for each proposal the user accepted —
/// mirrors `FusionProposal` but with `original_ids` instead of full
/// `Memory` rows, since `apply_fusion` only needs the ids to delete.
#[derive(Debug, Clone, Deserialize)]
pub struct FusionGroupInput {
    pub original_ids: Vec<String>,
    pub title: String,
    pub summary: Option<String>,
    pub content: String,
    pub category: String,
    pub importance: i64,
}

const VALID_CATEGORIES: [&str; 5] = ["person", "preference", "event", "fact", "other"];

/// No trailing `/no_think`: tried it (Qwen3's de-facto convention for
/// skipping hidden reasoning) to stop a small thinking-heavy local model
/// from burning its whole token budget on `reasoning_content` and never
/// reaching the JSON — but confirmed live that it back-fires on a
/// different, non-"thinking" Qwen3 checkpoint (`qwen3-vl-8b`): the exact
/// same prompt correctly grouped two near-duplicate memories and left
/// unrelated ones alone *without* the suffix, but short-circuited to
/// `{"groups": []}` in ~6 tokens *with* it, without visibly reasoning
/// either way (`reasoning_tokens: 0` in both runs) — the model just
/// pattern-matched the prompt's own empty-response example instead of
/// doing the comparison. Not a safe universal hack. `FUSION_MAX_TOKENS`
/// carries the actual fix (headroom for a model that does think) instead.
///
/// The worked example below replaced a plain "sé conservador" instruction
/// after live testing showed the plain version wasn't enough on its own:
/// with several similar-but-distinct memories in the same list (three
/// different coffee preferences plus one genuine near-duplicate), a
/// smaller/mid-size local model got overcautious and stopped proposing
/// *any* group at all, including the one genuine duplicate — a plain
/// "when in doubt, don't merge" instruction gives it nothing to calibrate
/// against once there's more than one similar-looking candidate. A
/// concrete positive/negative pair anchors the distinction it actually
/// needs to draw (same fact reworded vs. related-but-additive facts) and
/// fixed it in the same live test.
const FUSION_SYSTEM_PROMPT: &str = "Eres un organizador de memoria a largo plazo para un personaje de IA. \
Te doy una lista de memorias ya guardadas (id, título, resumen y categoría). Tu trabajo es encontrar grupos \
de memorias que sean redundantes o tan parecidas entre sí que convenga fusionarlas en una sola, más completa \
— nunca por estar en la misma categoría nada más, solo cuando hablan genuinamente de lo mismo (el mismo \
gusto, el mismo hecho, la misma persona, el mismo evento contado más de una vez). Si una memoria no tiene \
ningún par claro con el que fusionarse, no la incluyas en ningún grupo — dejarla afuera significa que queda \
tal cual está.\n\n\
Ejemplo de criterio:\n\
- \"Le gusta el café solo, sin azúcar\" y \"Prefiere el café sin nada, sin leche ni azúcar\" SÍ deben \
fusionarse: describen exactamente la misma preferencia con otras palabras.\n\
- \"Le gusta el café solo\" y \"Le gusta el café con leche\" NO deben fusionarse: son dos preferencias \
distintas y complementarias sobre el mismo tema, cada una aporta información nueva.\n\
- No fusiones solo por estar en la misma categoría. Fusiona solo cuando dos o más memorias son básicamente \
la misma información dicha de otra forma.\n\n\
Para cada grupo que propongas, da:\n\
- \"memory_ids\": la lista de ids de las memorias originales que este grupo reemplaza — como mínimo dos.\n\
- \"title\": título breve para la memoria combinada.\n\
- \"summary\": resumen compacto y autocontenido de la memoria combinada (una frase).\n\
- \"content\": versión detallada de la memoria combinada, incorporando lo importante de cada original sin \
repetir información.\n\
- \"category\": una de person, preference, event, fact, other.\n\
- \"importance\": 0-100, generalmente la más alta entre las originales que fusionás.\n\n\
Responde ÚNICAMENTE con un objeto JSON, sin ningún texto antes o después, con este formato exacto:\n\
{\"groups\": [{\"memory_ids\": [\"...\", \"...\"], \"title\": \"...\", \"summary\": \"...\", \"content\": \"...\", \
\"category\": \"person|preference|event|fact|other\", \"importance\": 0-100}]}\n\n\
Si ninguna memoria merece fusionarse, respondé exactamente: {\"groups\": []}";

#[derive(Debug, Clone, Deserialize)]
pub struct FusionGroupCandidate {
    #[serde(default)]
    pub memory_ids: Vec<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
    pub content: String,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub importance: Option<i64>,
}

impl FusionGroupCandidate {
    pub fn resolved_title(&self) -> String {
        match self.title.as_deref().map(str::trim) {
            Some(t) if !t.is_empty() => t.to_string(),
            _ => "Memoria fusionada".to_string(),
        }
    }

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
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct FusionResponse {
    #[serde(default)]
    pub groups: Vec<FusionGroupCandidate>,
}

pub fn system_prompt() -> String {
    FUSION_SYSTEM_PROMPT.to_string()
}

/// One line per memory, `id | título | resumen-o-contenido | categoría` —
/// dense enough that a local model can compare many memories at once
/// without burning context on each one's full `content`.
pub fn memories_listing(memories: &[Memory]) -> String {
    memories
        .iter()
        .map(|m| {
            let body = m.summary.as_deref().filter(|s| !s.trim().is_empty()).unwrap_or(&m.content);
            format!("- id={} | {} | {} | {}", m.id, m.title, body, m.category)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Same prose-tolerant parsing strategy as `extraction::parse_extraction_response`:
/// try the clean object shape, then an object substring within surrounding
/// prose. Never an error — a response that doesn't parse just means no
/// fusion proposals this pass, exactly like an unparseable extraction
/// response yields no memories.
pub fn parse_fusion_response(raw: &str) -> Option<FusionResponse> {
    let trimmed = raw.trim();

    if let Ok(response) = serde_json::from_str::<FusionResponse>(trimmed) {
        return Some(response);
    }
    if let Some(response) = extract_delimited(trimmed, '{', '}').and_then(|s| serde_json::from_str::<FusionResponse>(s).ok()) {
        return Some(response);
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
        let raw = r#"{"groups": [{"memory_ids": ["a", "b"], "title": "Café", "summary": "Le gusta el café",
            "content": "Le gusta el café, tanto solo como con leche", "category": "preference", "importance": 60}]}"#;
        let response = parse_fusion_response(raw).unwrap();
        assert_eq!(response.groups.len(), 1);
        assert_eq!(response.groups[0].memory_ids, vec!["a", "b"]);
        assert_eq!(response.groups[0].resolved_title(), "Café");
        assert_eq!(response.groups[0].normalized_category(), "preference");
    }

    #[test]
    fn an_empty_groups_response_parses_to_nothing() {
        let response = parse_fusion_response(r#"{"groups": []}"#).unwrap();
        assert!(response.groups.is_empty());
    }

    #[test]
    fn extracts_the_object_even_with_surrounding_prose() {
        let raw = "Claro, aquí está el análisis:\n{\"groups\": [{\"memory_ids\": [\"a\",\"b\"], \"content\": \"algo\"}]}\nEspero que ayude.";
        let response = parse_fusion_response(raw).unwrap();
        assert_eq!(response.groups.len(), 1);
    }

    #[test]
    fn genuinely_unparseable_text_returns_none() {
        assert!(parse_fusion_response("lo siento, no puedo ayudar con eso").is_none());
    }

    #[test]
    fn missing_title_falls_back_to_a_generic_label() {
        let raw = r#"{"groups": [{"memory_ids": ["a", "b"], "content": "algo"}]}"#;
        let response = parse_fusion_response(raw).unwrap();
        assert_eq!(response.groups[0].resolved_title(), "Memoria fusionada");
    }

    #[test]
    fn an_unknown_category_falls_back_to_other() {
        let raw = r#"{"groups": [{"memory_ids": ["a", "b"], "content": "algo", "category": "chisme"}]}"#;
        let response = parse_fusion_response(raw).unwrap();
        assert_eq!(response.groups[0].normalized_category(), "other");
    }

    #[test]
    fn out_of_range_importance_is_clamped() {
        let raw = r#"{"groups": [{"memory_ids": ["a", "b"], "content": "algo", "importance": 500}]}"#;
        let response = parse_fusion_response(raw).unwrap();
        assert_eq!(response.groups[0].clamped_importance(), 100);
    }

    #[test]
    fn memories_listing_renders_one_line_per_memory_preferring_summary_over_content() {
        let memories = vec![sample_memory("m1", "Café", Some("Le gusta el café"), "Le gusta mucho el café con leche")];
        let listing = memories_listing(&memories);
        assert!(listing.contains("id=m1"));
        assert!(listing.contains("Le gusta el café"));
        assert!(!listing.contains("con leche"));
    }

    fn sample_memory(id: &str, title: &str, summary: Option<&str>, content: &str) -> Memory {
        Memory {
            id: id.to_string(),
            profile_id: "p1".to_string(),
            title: title.to_string(),
            content: content.to_string(),
            summary: summary.map(str::to_string),
            category: "preference".to_string(),
            importance: 50,
            confidence: 50,
            source: "manual".to_string(),
            source_id: None,
            is_pinned: false,
            scope: "private".to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
            last_accessed_at: None,
            access_count: 0,
        }
    }
}
