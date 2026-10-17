//! Per-turn conversation policy: how the character should talk *this* turn.
//!
//! Computed locally from the channel and the user's last message (no extra
//! LLM call). Identity and personality say who the character is; this says
//! how a companion converses: chat first, help only when asked, short in a
//! voice call. Mood and personality sit below it, so they change the tone,
//! never the length.

/// Where the reply will be delivered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Channel {
    #[default]
    Chat,
    Call,
    Twitch,
    Discord,
}

/// Instruction block for this turn, in the app language (unknown falls back to Spanish).
pub fn instruction(channel: Channel, last_user_text: &str, lang: &str) -> String {
    let t = texts(lang);
    let mut out = t.base.to_string();
    if is_help_request(last_user_text) {
        out.push(' ');
        out.push_str(t.help);
    } else if weighted_len(last_user_text) <= TINY_MESSAGE {
        out.push(' ');
        out.push_str(t.tiny);
    }
    if channel == Channel::Call {
        out.push(' ');
        out.push_str(t.voice);
    }
    out
}

/// Short messages ("jajaja sii", "tengo sueño") get a one-line reply.
const TINY_MESSAGE: usize = 30;
const LONG_MESSAGE: usize = 200;

/// Character count where CJK characters weigh 3: one of them carries about a word.
fn weighted_len(text: &str) -> usize {
    text.trim().chars().map(|c| if (c as u32) >= 0x2E80 { 3 } else { 1 }).sum()
}

const HELP_WORDS: &[&str] = &[
    "explica", "ayuda", "ayúdame", "cómo funciona", "cómo se ", "how do", "how to", "how does", "explain", "help me", "ajuda",
    "como funciona", "como se ", "説明", "教えて", "설명", "알려", "解释", "怎么", "如何", "帮我",
];

fn is_help_request(text: &str) -> bool {
    let lower = text.to_lowercase();
    weighted_len(text) > LONG_MESSAGE || lower.contains("```") || HELP_WORDS.iter().any(|w| lower.contains(w))
}

struct Texts {
    base: &'static str,
    tiny: &'static str,
    help: &'static str,
    voice: &'static str,
}

fn texts(lang: &str) -> Texts {
    match lang {
        "en" => Texts {
            base: "Your main role here is to converse and keep company, not to act as an assistant. Answer as part of an ongoing chat: react first to what they said, match the length to their message (in casual talk a sentence or a few is enough), no lists or headings unless asked, don't end by offering help, and don't ask a question every turn. You can joke, give an opinion, disagree or just comment.",
            tiny: "Their message is very short: reply with one short sentence.",
            help: "This message asks for help or an explanation: be clear and give the detail needed, and you may structure the answer.",
            voice: "This is a spoken conversation: short sentences that are easy to say aloud, usually one or two, with no Markdown, lists or symbols.",
        },
        "pt" => Texts {
            base: "Sua função principal aqui é conversar e fazer companhia, não agir como assistente. Responda como parte de uma conversa contínua: reaja primeiro ao que disseram, ajuste o tamanho à mensagem (em papo casual uma ou poucas frases bastam), sem listas nem títulos a menos que peçam, não termine oferecendo ajuda e não faça uma pergunta a cada turno. Você pode brincar, opinar, discordar ou só comentar.",
            tiny: "A mensagem é muito curta: responda com uma frase curta.",
            help: "Esta mensagem pede ajuda ou uma explicação: seja claro e dê o detalhe necessário, e pode estruturar a resposta.",
            voice: "Esta é uma conversa falada: frases curtas e fáceis de pronunciar, normalmente uma ou duas, sem Markdown, listas nem símbolos.",
        },
        "ja" => Texts {
            base: "ここでのあなたの主な役割は、アシスタントとして振る舞うことではなく、会話して寄り添うことです。続いている会話の一部として答えてください。まず相手の言ったことに反応し、長さは相手のメッセージに合わせ、雑談なら一文か数文で十分です。頼まれない限り箇条書きや見出しは使わず、手伝いを申し出て締めくくらず、毎回質問もしないでください。冗談を言ったり、意見を述べたり、反対したり、ただ感想を言ってもかまいません。",
            tiny: "相手のメッセージはとても短いので、短い一文で返してください。",
            help: "このメッセージは助けや説明を求めています。分かりやすく、必要なだけ詳しく答え、構造化してもかまいません。",
            voice: "これは音声での会話です。声に出しやすい短い文で、たいてい一、二文にし、Markdownや箇条書きや記号は使わないでください。",
        },
        "ko" => Texts {
            base: "여기서 당신의 주된 역할은 어시스턴트가 아니라 대화하고 곁을 지켜 주는 것입니다. 이어지는 대화의 일부로 답하세요. 먼저 상대가 한 말에 반응하고, 길이는 상대의 메시지에 맞추며, 가벼운 대화에서는 한두 문장이면 충분합니다. 요청이 없으면 목록이나 제목을 쓰지 말고, 도와주겠다는 말로 끝맺지 말고, 매번 질문하지 마세요. 농담하거나 의견을 말하거나 반대하거나 그냥 한마디 덧붙여도 됩니다.",
            tiny: "상대의 메시지가 아주 짧으니 짧은 한 문장으로 답하세요.",
            help: "이 메시지는 도움이나 설명을 요청합니다. 명확하게, 필요한 만큼 자세히 답하고 구조화해도 됩니다.",
            voice: "이것은 음성 대화입니다. 소리 내어 말하기 쉬운 짧은 문장으로, 보통 한두 문장만 말하고 Markdown, 목록, 기호는 쓰지 마세요.",
        },
        "zh" => Texts {
            base: "你在这里的主要角色是聊天和陪伴，而不是充当助手。像持续进行的对话一样回答：先对对方说的话做出反应，长度与对方的消息相称，闲聊时一句或几句就够了；除非被要求，不要使用列表或标题，不要以主动提供帮助作结，也不要每轮都提问。你可以开玩笑、发表看法、表示不同意，或者只是随口评论。",
            tiny: "对方的消息很短，请用一句简短的话回答。",
            help: "这条消息是在求助或要求解释：请清楚地回答并给出所需的细节，可以分点组织。",
            voice: "这是语音对话：用容易说出口的短句，通常一两句，不要使用 Markdown、列表或符号。",
        },
        _ => Texts {
            base: "Tu función principal aquí es conversar y acompañar, no actuar como asistente. Responde como parte de una charla continua: reacciona primero a lo que te dicen, adapta la longitud al mensaje (en charla casual bastan una o pocas frases), no uses listas ni encabezados salvo que los pidan, no cierres ofreciendo ayuda y no hagas una pregunta en cada turno. Puedes bromear, opinar, discrepar o simplemente comentar.",
            tiny: "Su mensaje es muy breve: responde con una frase corta.",
            help: "Este mensaje pide ayuda o una explicación: sé claro y da el detalle necesario, y puedes estructurar la respuesta.",
            voice: "Esto es una conversación hablada: frases cortas y fáciles de pronunciar, normalmente una o dos, sin Markdown, listas ni símbolos.",
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tiny_casual_message_asks_for_one_short_sentence() {
        let text = instruction(Channel::Chat, "jajaja sii", "es");
        assert!(text.contains("frase corta"));
        assert!(!text.contains("estructurar"));
    }

    #[test]
    fn a_help_request_allows_detail_even_when_short() {
        let text = instruction(Channel::Chat, "explica los embeddings", "es");
        assert!(text.contains("estructurar"));
        assert!(!text.contains("frase corta"));
    }

    #[test]
    fn a_long_message_is_treated_as_a_help_request() {
        assert!(instruction(Channel::Chat, &"palabra ".repeat(40), "es").contains("estructurar"));
    }

    #[test]
    fn only_a_call_gets_the_spoken_rule() {
        assert!(instruction(Channel::Call, "cuéntame de tu día, ¿qué hiciste?", "es").contains("conversación hablada"));
        assert!(!instruction(Channel::Twitch, "cuéntame de tu día, ¿qué hiciste?", "es").contains("conversación hablada"));
    }

    #[test]
    fn cjk_counts_more_per_character() {
        assert!(instruction(Channel::Chat, "うん", "ja").contains("短い一文"));
        assert!(!instruction(Channel::Chat, "今日は友達と一緒に新しいカフェに行ってきました", "ja").contains("短い一文"));
    }

    #[test]
    fn unknown_language_falls_back_to_spanish() {
        assert!(instruction(Channel::Chat, "hola", "xx").contains("conversar y acompañar"));
    }
}
