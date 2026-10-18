//! What the character should remember about *this conversation* beyond the recent
//! history: a running summary of what already left the history window, and the
//! threads still open. Both are written by the model itself (see the chat service),
//! so the text tells it to treat them as notes that may be imperfect.

pub struct SessionContext<'a> {
    pub summary: &'a str,
    pub open_threads: &'a [String],
}

impl SessionContext<'_> {
    /// The block added to the system prompt, in the app language; `None` when empty.
    pub fn render(&self, lang: &str) -> Option<String> {
        let summary = self.summary.trim();
        let threads: Vec<&str> = self.open_threads.iter().map(|t| t.trim()).filter(|t| !t.is_empty()).collect();
        if summary.is_empty() && threads.is_empty() {
            return None;
        }
        let t = texts(lang);
        let mut out = String::new();
        if !summary.is_empty() {
            out.push_str(&format!("{}\n{summary}", t.summary));
        }
        if !threads.is_empty() {
            if !out.is_empty() {
                out.push_str("\n\n");
            }
            out.push_str(t.threads);
            for thread in threads {
                out.push_str(&format!("\n- {thread}"));
            }
        }
        out.push_str("\n\n");
        out.push_str(t.caution);
        Some(out)
    }
}

struct Texts {
    summary: &'static str,
    threads: &'static str,
    caution: &'static str,
}

fn texts(lang: &str) -> Texts {
    match lang {
        "en" => Texts {
            summary: "Summary of what was said earlier in this conversation:",
            threads: "Topics left pending in this conversation:",
            caution: "These are your own notes (in them, “the character” is you and “the user” is the person you are talking to) and may be imperfect: use them as background, don't recite them, and don't state as fact anything the notes don't make clear. You may bring up a pending topic naturally when it fits.",
        },
        "pt" => Texts {
            summary: "Resumo do que foi dito antes nesta conversa:",
            threads: "Assuntos pendentes desta conversa:",
            caution: "São anotações suas (nelas, “o personagem” é você e “o usuário” é a pessoa com quem você fala) e podem ser imperfeitas: use-as como contexto, não as recite e não afirme como fato o que elas não deixam claro. Você pode retomar um assunto pendente com naturalidade quando fizer sentido.",
        },
        "ja" => Texts {
            summary: "この会話でこれまでに話した内容の要約：",
            threads: "この会話で保留になっている話題：",
            caution: "これはあなた自身のメモです（メモ中の「キャラクター」はあなた、「ユーザー」は話し相手です）。不正確な場合があります。背景として使い、そのまま読み上げず、メモから明確でないことを事実として述べないでください。保留の話題は、自然な流れのときに触れてもかまいません。",
        },
        "ko" => Texts {
            summary: "이 대화에서 앞서 나눈 내용의 요약:",
            threads: "이 대화에서 아직 남아 있는 주제:",
            caution: "이것은 당신 자신의 메모입니다(메모에서 “캐릭터”는 당신, “사용자”는 대화 상대입니다). 부정확할 수 있습니다. 배경 정보로만 활용하고, 그대로 읽지 말고, 메모에서 분명하지 않은 내용을 사실처럼 말하지 마세요. 남은 주제는 자연스러울 때 꺼내도 됩니다.",
        },
        "zh" => Texts {
            summary: "本次对话中此前内容的摘要：",
            threads: "本次对话中尚未了结的话题：",
            caution: "这些是你自己的笔记（笔记中的“角色”是你，“用户”是和你聊天的人），可能并不完美：把它们当作背景，不要照着念，也不要把笔记里不明确的内容当作事实。合适时可以自然地重提尚未了结的话题。",
        },
        _ => Texts {
            summary: "Resumen de lo que se dijo antes en esta conversación:",
            threads: "Temas que quedaron pendientes en esta conversación:",
            caution: "Son notas tuyas (en ellas, «el personaje» eres tú y «el usuario» es la persona con la que hablas) y pueden ser imperfectas: úsalas como contexto, no las recites y no afirmes como hecho lo que las notas no dejan claro. Puedes retomar un tema pendiente con naturalidad cuando encaje.",
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_context_renders_nothing() {
        assert!(SessionContext { summary: "  ", open_threads: &[] }.render("es").is_none());
        assert!(SessionContext { summary: "", open_threads: &["  ".to_string()] }.render("es").is_none());
    }

    #[test]
    fn summary_and_threads_render_in_the_app_language() {
        let threads = ["Mañana tiene examen".to_string()];
        let es = SessionContext { summary: "Hablaron de un viaje.", open_threads: &threads }.render("es").unwrap();
        assert!(es.contains("Resumen de lo que se dijo") && es.contains("Hablaron de un viaje.") && es.contains("- Mañana tiene examen"));
        let en = SessionContext { summary: "They talked about a trip.", open_threads: &[] }.render("en").unwrap();
        assert!(en.contains("Summary of what was said") && !en.contains("pending in this conversation"));
    }
}
