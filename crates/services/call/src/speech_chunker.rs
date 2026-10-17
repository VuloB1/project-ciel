//! Cuts the LLM's streamed text into sentences so the call can start speaking the
//! first one while the model is still writing the rest.

/// A sentence shorter than this keeps accumulating, so playback isn't choppy.
const MIN_SENTENCE_CHARS: usize = 12;

#[derive(Default)]
pub struct SpeechChunker {
    buffer: String,
}

impl SpeechChunker {
    /// Adds a streamed delta and returns the sentences that are now complete.
    pub fn push(&mut self, delta: &str) -> Vec<String> {
        self.buffer.push_str(delta);
        let mut sentences = Vec::new();
        while let Some(end) = self.boundary() {
            let rest = self.buffer.split_off(end);
            let sentence = std::mem::replace(&mut self.buffer, rest);
            let sentence = sentence.trim();
            if !sentence.is_empty() {
                sentences.push(sentence.to_string());
            }
        }
        sentences
    }

    /// Whatever is left once the model has finished.
    pub fn finish(&mut self) -> Option<String> {
        let rest = std::mem::take(&mut self.buffer);
        let rest = rest.trim();
        (!rest.is_empty()).then(|| rest.to_string())
    }

    /// Byte index just after the first usable sentence end, if there is one. A Latin
    /// terminator only counts when whitespace already follows it (so "3.5" or a "."
    /// split across two deltas is not cut); CJK terminators count immediately.
    fn boundary(&self) -> Option<usize> {
        let mut chars = self.buffer.char_indices().peekable();
        while let Some((i, c)) = chars.next() {
            let end = i + c.len_utf8();
            let terminator = match c {
                '。' | '！' | '？' | '…' => true,
                '.' | '!' | '?' | '\n' => chars.peek().is_some_and(|(_, next)| next.is_whitespace()),
                _ => false,
            };
            if terminator && self.buffer[..end].trim().chars().count() >= MIN_SENTENCE_CHARS {
                return Some(end);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(deltas: &[&str]) -> Vec<String> {
        let mut chunker = SpeechChunker::default();
        let mut out: Vec<String> = deltas.iter().flat_map(|d| chunker.push(d)).collect();
        out.extend(chunker.finish());
        out
    }

    #[test]
    fn splits_on_sentence_ends_once_whitespace_follows() {
        assert_eq!(run(&["Hola, ¿cómo estás? ", "Yo bien, gracias por preguntar. Y tú"]), vec!["Hola, ¿cómo estás?", "Yo bien, gracias por preguntar.", "Y tú"]);
    }

    #[test]
    fn a_terminator_at_the_end_of_a_delta_waits_for_the_next_one() {
        let mut chunker = SpeechChunker::default();
        assert!(chunker.push("Mide 3.").is_empty());
        assert!(chunker.push("5 metros de largo y es muy alto.").is_empty());
        assert_eq!(chunker.finish().unwrap(), "Mide 3.5 metros de largo y es muy alto.");
    }

    #[test]
    fn short_sentences_merge_with_the_next_one() {
        assert_eq!(run(&["Sí. Claro que me acuerdo de eso. Fue genial."]), vec!["Sí. Claro que me acuerdo de eso.", "Fue genial."]);
    }

    #[test]
    fn cjk_terminators_split_without_whitespace() {
        assert_eq!(run(&["今日はとても良い天気ですね。散歩に行きたいです。"]), vec!["今日はとても良い天気ですね。", "散歩に行きたいです。"]);
    }

    #[test]
    fn empty_input_yields_nothing() {
        assert!(run(&["", "  "]).is_empty());
    }

    #[test]
    fn nothing_is_lost_or_duplicated() {
        let text = "Primera frase larga aquí. Segunda frase también larga! Tercera sin cierre";
        assert_eq!(run(&[text]).join(" "), text);
    }
}
