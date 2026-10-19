//! Cuts the LLM's streamed text into sentences so the call can start speaking the
//! first one while the model is still writing the rest.

/// A sentence shorter than this keeps accumulating, so playback isn't choppy.
const MIN_SENTENCE_CHARS: usize = 12;

#[derive(Default)]
pub struct SpeechChunker {
    buffer: String,
    /// Cut a question that closes the reply (see `drops_closing_question`).
    drop_closing_question: bool,
    /// A question sentence waiting to learn whether it is the last one.
    held: Option<String>,
    emitted: bool,
    dropped: Option<String>,
}

impl SpeechChunker {
    pub fn new(drop_closing_question: bool) -> Self {
        Self { drop_closing_question, ..Self::default() }
    }

    /// Adds a streamed delta and returns the sentences that are now complete.
    pub fn push(&mut self, delta: &str) -> Vec<String> {
        self.buffer.push_str(delta);
        let mut sentences = Vec::new();
        while let Some(end) = self.boundary() {
            let rest = self.buffer.split_off(end);
            let sentence = std::mem::replace(&mut self.buffer, rest);
            let sentence = sentence.trim();
            if !sentence.is_empty() {
                self.route(sentence.to_string(), &mut sentences);
            }
        }
        sentences
    }

    /// Whatever is left once the model has finished. A held closing question is dropped,
    /// unless it is all there is (an empty reply would be worse).
    pub fn finish(&mut self) -> Vec<String> {
        let mut sentences = Vec::new();
        let rest = std::mem::take(&mut self.buffer);
        let rest = rest.trim();
        if !rest.is_empty() {
            self.route(rest.to_string(), &mut sentences);
        }
        if let Some(held) = self.held.take() {
            if self.emitted || !sentences.is_empty() {
                self.dropped = Some(held);
            } else {
                sentences.push(held);
            }
        }
        sentences
    }

    /// The closing question `finish` cut, if any, so the caller can cut it from the stored text too.
    pub fn dropped(&self) -> Option<&str> {
        self.dropped.as_deref()
    }

    /// Questions are held back one sentence when dropping is on; everything else flows at once.
    fn route(&mut self, sentence: String, out: &mut Vec<String>) {
        out.extend(self.held.take());
        if self.drop_closing_question && aivt_orchestrator::ends_with_question(&sentence) {
            self.held = Some(sentence);
        } else {
            out.push(sentence);
        }
        self.emitted |= !out.is_empty();
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
        run_with(false, deltas)
    }

    fn run_with(drop_closing_question: bool, deltas: &[&str]) -> Vec<String> {
        let mut chunker = SpeechChunker::new(drop_closing_question);
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
        assert_eq!(chunker.finish(), vec!["Mide 3.5 metros de largo y es muy alto."]);
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

    #[test]
    fn a_closing_question_is_cut_only_when_asked_to() {
        let text = ["Qué buen plan, me encanta. ¿Y tú qué harás luego? "];
        assert_eq!(run_with(true, &text), vec!["Qué buen plan, me encanta."]);
        assert_eq!(run_with(false, &text), vec!["Qué buen plan, me encanta.", "¿Y tú qué harás luego?"]);
    }

    #[test]
    fn a_question_in_the_middle_is_kept_and_a_lone_question_survives() {
        assert_eq!(run_with(true, &["¿Sabes qué pasó ayer? Me quedé dormida. Fue genial."]), vec!["¿Sabes qué pasó ayer?", "Me quedé dormida.", "Fue genial."]);
        assert_eq!(run_with(true, &["¿Cómo te fue en el trabajo?"]), vec!["¿Cómo te fue en el trabajo?"]);
    }

    #[test]
    fn the_cut_question_is_reported() {
        let mut chunker = SpeechChunker::new(true);
        let mut all = chunker.push("Qué bien, me alegro mucho. ¿Y tú qué tal estás? ");
        all.extend(chunker.finish());
        assert_eq!(all, vec!["Qué bien, me alegro mucho."]);
        assert_eq!(chunker.dropped(), Some("¿Y tú qué tal estás?"));
    }
}
