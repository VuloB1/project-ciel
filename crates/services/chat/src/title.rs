const MAX_TITLE_LENGTH: usize = 48;

/// Derives a conversation title from its first message (sección 14 del
/// brief: "generar automáticamente el título basándose en los primeros
/// mensajes"). Truncates at a word boundary when there's a reasonable one
/// nearby, rather than always cutting mid-word.
pub(crate) fn title_from_first_message(content: &str) -> String {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return "Nueva conversación".to_string();
    }
    if trimmed.chars().count() <= MAX_TITLE_LENGTH {
        return trimmed.to_string();
    }

    let mut truncated: String = trimmed.chars().take(MAX_TITLE_LENGTH).collect();
    if let Some(last_space) = truncated.rfind(' ') {
        if last_space > MAX_TITLE_LENGTH / 2 {
            truncated.truncate(last_space);
        }
    }
    format!("{}…", truncated.trim_end())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_message_becomes_the_title_verbatim() {
        assert_eq!(title_from_first_message("Hola, ¿cómo estás?"), "Hola, ¿cómo estás?");
    }

    #[test]
    fn empty_message_falls_back_to_a_default_title() {
        assert_eq!(title_from_first_message("   "), "Nueva conversación");
    }

    #[test]
    fn long_message_is_truncated_at_a_word_boundary_with_ellipsis() {
        let content = "Quiero que me ayudes a planear un viaje de tres semanas por el sur de Argentina";
        let title = title_from_first_message(content);
        assert!(title.ends_with('…'));
        assert!(title.chars().count() <= MAX_TITLE_LENGTH + 1);
        assert!(!title.contains("  "));
    }

    #[test]
    fn a_single_very_long_word_is_hard_truncated() {
        let content = "a".repeat(100);
        let title = title_from_first_message(&content);
        assert!(title.ends_with('…'));
        assert_eq!(title.chars().count(), MAX_TITLE_LENGTH + 1);
    }
}
