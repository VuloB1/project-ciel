use unicode_normalization::UnicodeNormalization;

/// Case-insensitive, diacritic-insensitive whole-word search — `word` must
/// appear as its own token (split on non-alphanumeric boundaries) so e.g. a
/// character named "Mia" doesn't fire on every "estaría", but punctuation
/// around it ("@Ciel", "Ciel,", "oye Ciel!") still matches.
///
/// Diacritics (Spanish accents/tildes, Japanese-romanization macrons, etc.)
/// are stripped from both sides before comparing, so a profile named e.g.
/// "Ryūji" or "León" is still recognized when Whisper's transcript comes
/// back without the mark ("Ryuji", "Leon") — the much more likely outcome,
/// since STT models are inconsistent about reproducing diacritics even with
/// prompt-priming (`CallService::transcription_prompt`) pointed right at
/// the correct spelling. Pedido del usuario: los perfiles pueden llevar
/// cualquier nombre/apodo (incluyendo japoneses), y el reconocimiento debe
/// funcionar de forma general, no solo para casos probados a mano como
/// "Ciel".
///
/// Shared across every channel that needs "does this text address the
/// character by name" — Twitch chat (`mentioned_only` mode, since a
/// viewer can't officially @-mention a character that has no Twitch
/// account of its own) and Llamada (an optional wake-phrase gate so the
/// character doesn't reply to every single utterance during an open call —
/// see `aivt_service_call`).
pub fn contains_word(text: &str, word: &str) -> bool {
    let word = word.trim();
    if word.is_empty() {
        return false;
    }
    let word_folded = fold(word);
    text.split(|c: char| !c.is_alphanumeric()).any(|token| !token.is_empty() && fold(token) == word_folded)
}

/// Lowercases and strips Unicode combining diacritical marks (NFD
/// decomposition, then drop anything in the U+0300–U+036F combining-marks
/// block) — covers Spanish accents/tildes and Japanese-romanization
/// macrons alike, since both decompose into a base letter plus a mark in
/// that block.
fn fold(s: &str) -> String {
    s.to_lowercase().nfd().filter(|c| !('\u{0300}'..='\u{036f}').contains(c)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_case_insensitively_around_punctuation() {
        assert!(contains_word("oye @Ciel, hola!", "ciel"));
        assert!(contains_word("CIEL responde", "Ciel"));
        assert!(!contains_word("estaría bien", "ia"));
        assert!(!contains_word("hola mundo", "ciel"));
    }

    #[test]
    fn an_empty_word_never_matches() {
        assert!(!contains_word("cualquier texto", ""));
        assert!(!contains_word("cualquier texto", "   "));
    }

    #[test]
    fn matches_regardless_of_which_side_carries_the_accent() {
        // Perfil "León" pero Whisper transcribió sin tilde.
        assert!(contains_word("oye Leon, hola", "León"));
        // Y al revés: el usuario escribió el apodo con tilde pero el habla
        // se transcribió igual sin ella.
        assert!(contains_word("oye León, hola", "Leon"));
    }

    #[test]
    fn matches_romanized_japanese_names_with_or_without_macrons() {
        assert!(contains_word("¿qué opinas Ryuji?", "Ryūji"));
        assert!(contains_word("¿qué opinas Ryūji?", "Ryuji"));
    }
}
