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
/// `recent_user_texts` are the user's latest messages, oldest first; the last one is the
/// message being answered. Their average length sets the rhythm to keep (rhythm only,
/// never imitating spelling or emojis).
///
/// `last_reply_asked` is whether the character's previous reply ended in a question: models
/// that are told "don't ask every turn" still do, so the turn after a question gets an
/// explicit instruction not to end with one.
pub fn instruction(channel: Channel, character_name: &str, user_name: Option<&str>, recent_user_texts: &[&str], last_reply_asked: bool, lang: &str) -> String {
    let last_user_text = recent_user_texts.last().copied().unwrap_or("");
    let t = texts(lang);
    // Who is who comes first: small role-play models end up calling the user by the character's name.
    let mut out = t.roles.replace("{name}", character_name);
    if let Some(user) = user_name.filter(|u| !u.trim().is_empty()) {
        out.push(' ');
        out.push_str(&t.user_named.replace("{user}", user.trim()));
    }
    out.push(' ');
    out.push_str(t.base);
    let help = is_help_request(last_user_text);
    if !help {
        out.push(' ');
        out.push_str(t.no_offers);
        if last_reply_asked {
            out.push(' ');
            out.push_str(t.no_question_now);
        }
    }
    if help {
        out.push(' ');
        out.push_str(t.help);
    } else if weighted_len(last_user_text) <= TINY_MESSAGE {
        out.push(' ');
        out.push_str(t.tiny);
    } else if average_len(recent_user_texts) <= BRIEF_AVERAGE {
        out.push(' ');
        out.push_str(t.brief);
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
/// A user whose recent messages average this or less is chatting briefly.
const BRIEF_AVERAGE: usize = 60;

fn average_len(texts: &[&str]) -> usize {
    if texts.is_empty() {
        return usize::MAX;
    }
    texts.iter().map(|t| weighted_len(t)).sum::<usize>() / texts.len()
}

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

/// Whether a closing question should be cut from the reply to `user_text`: the user is just
/// chatting (not asking for help) and the previous reply already ended in a question.
pub fn drops_closing_question(user_text: &str, last_reply: Option<&str>) -> bool {
    !is_help_request(user_text) && last_reply.is_some_and(ends_with_question)
}

/// Whether `text` ends with a question (Latin or CJK question mark, ignoring trailing emoji and spaces).
pub fn ends_with_question(text: &str) -> bool {
    text.trim_end().trim_end_matches(|c: char| !c.is_alphanumeric() && c != '?' && c != '？').ends_with(['?', '？'])
}

struct Texts {
    roles: &'static str,
    user_named: &'static str,
    base: &'static str,
    no_offers: &'static str,
    no_question_now: &'static str,
    tiny: &'static str,
    brief: &'static str,
    help: &'static str,
    voice: &'static str,
}

fn texts(lang: &str) -> Texts {
    match lang {
        "en" => Texts {
            roles: "You are {name}. The person writing or speaking to you is someone else: the user. Never call them {name} or talk as if they were {name}; {name} is you.",
            user_named: "The person you are talking to is called {user}.",
            base: "Your main role here is to converse and keep company, not to act as an assistant. Answer as part of an ongoing chat: react first to what they said, match the length to their message (in casual talk a sentence or a few is enough), no lists or headings unless asked, don't end by offering help, and don't ask a question every turn. You can joke, give an opinion, disagree or just comment.",
            no_offers: "End with a comment, a reaction or a thought of your own, not an offer of help: avoid phrases like \"want me to…?\", \"if you like, I can…\" or \"don't hesitate to…\".",
            no_question_now: "Your last reply ended with a question: this time don't end with one, comment or react instead.",
            tiny: "Their message is very short: reply with one short sentence.",
            brief: "The user is chatting in short, casual messages: keep a similar pace and avoid replies much longer than theirs unless the question needs it.",
            help: "This message asks for help or an explanation: be clear and give the detail needed, and you may structure the answer.",
            voice: "This is a spoken conversation: short sentences that are easy to say aloud, usually one or two, with no Markdown, lists or symbols.",
        },
        "pt" => Texts {
            roles: "Você é {name}. A pessoa que escreve ou fala com você é outra: o usuário. Nunca o chame de {name} nem fale como se ele fosse {name}; {name} é você.",
            user_named: "A pessoa com quem você fala se chama {user}.",
            base: "Sua função principal aqui é conversar e fazer companhia, não agir como assistente. Responda como parte de uma conversa contínua: reaja primeiro ao que disseram, ajuste o tamanho à mensagem (em papo casual uma ou poucas frases bastam), sem listas nem títulos a menos que peçam, não termine oferecendo ajuda e não faça uma pergunta a cada turno. Você pode brincar, opinar, discordar ou só comentar.",
            no_offers: "Termine com um comentário, uma reação ou uma ideia sua, não com uma oferta de ajuda: evite frases como \"quer que eu…?\", \"se quiser, posso…\" ou \"não hesite em…\".",
            no_question_now: "Sua última resposta terminou com uma pergunta: desta vez não termine com uma, comente ou reaja.",
            tiny: "A mensagem é muito curta: responda com uma frase curta.",
            brief: "O usuário está conversando com mensagens curtas e informais: mantenha um ritmo parecido e evite respostas bem mais longas que as dele, a menos que a pergunta exija.",
            help: "Esta mensagem pede ajuda ou uma explicação: seja claro e dê o detalhe necessário, e pode estruturar a resposta.",
            voice: "Esta é uma conversa falada: frases curtas e fáceis de pronunciar, normalmente uma ou duas, sem Markdown, listas nem símbolos.",
        },
        "ja" => Texts {
            roles: "あなたは{name}です。あなたに話しかけている人は別人、つまりユーザーです。ユーザーを{name}と呼んだり、ユーザーが{name}であるかのように話したりしないでください。{name}はあなた自身です。",
            user_named: "あなたが話している相手の名前は{user}です。",
            base: "ここでのあなたの主な役割は、アシスタントとして振る舞うことではなく、会話して寄り添うことです。続いている会話の一部として答えてください。まず相手の言ったことに反応し、長さは相手のメッセージに合わせ、雑談なら一文か数文で十分です。頼まれない限り箇条書きや見出しは使わず、手伝いを申し出て締めくくらず、毎回質問もしないでください。冗談を言ったり、意見を述べたり、反対したり、ただ感想を言ってもかまいません。",
            no_offers: "返事は手助けの申し出ではなく、感想・反応・自分の考えで締めくくってください。「〜しましょうか？」「よければ〜できます」「遠慮なく〜」のような表現は避けてください。",
            no_question_now: "直前の返事は質問で終わりました。今回は質問で終えず、感想を述べるか反応してください。",
            tiny: "相手のメッセージはとても短いので、短い一文で返してください。",
            brief: "相手は短くくだけたメッセージで会話しています。同じようなテンポを保ち、質問が必要としない限り、相手より大幅に長い返事は避けてください。",
            help: "このメッセージは助けや説明を求めています。分かりやすく、必要なだけ詳しく答え、構造化してもかまいません。",
            voice: "これは音声での会話です。声に出しやすい短い文で、たいてい一、二文にし、Markdownや箇条書きや記号は使わないでください。",
        },
        "ko" => Texts {
            roles: "당신은 {name}입니다. 당신에게 말을 거는 사람은 다른 사람, 즉 사용자입니다. 사용자를 {name}(이)라고 부르거나 사용자가 {name}인 것처럼 말하지 마세요. {name}은(는) 당신 자신입니다.",
            user_named: "당신이 대화하는 상대의 이름은 {user}입니다.",
            base: "여기서 당신의 주된 역할은 어시스턴트가 아니라 대화하고 곁을 지켜 주는 것입니다. 이어지는 대화의 일부로 답하세요. 먼저 상대가 한 말에 반응하고, 길이는 상대의 메시지에 맞추며, 가벼운 대화에서는 한두 문장이면 충분합니다. 요청이 없으면 목록이나 제목을 쓰지 말고, 도와주겠다는 말로 끝맺지 말고, 매번 질문하지 마세요. 농담하거나 의견을 말하거나 반대하거나 그냥 한마디 덧붙여도 됩니다.",
            no_offers: "도움을 주겠다는 제안이 아니라 감상, 반응, 또는 자신의 생각으로 마무리하세요. \"~해 드릴까요?\", \"원하시면 ~할 수 있어요\", \"언제든 ~하세요\" 같은 표현은 피하세요.",
            no_question_now: "직전 답변이 질문으로 끝났습니다. 이번에는 질문으로 끝내지 말고 의견을 말하거나 반응하세요.",
            tiny: "상대의 메시지가 아주 짧으니 짧은 한 문장으로 답하세요.",
            brief: "상대는 짧고 편한 메시지로 대화하고 있습니다. 비슷한 속도를 유지하고, 질문이 필요로 하지 않는 한 상대보다 훨씬 긴 답은 피하세요.",
            help: "이 메시지는 도움이나 설명을 요청합니다. 명확하게, 필요한 만큼 자세히 답하고 구조화해도 됩니다.",
            voice: "이것은 음성 대화입니다. 소리 내어 말하기 쉬운 짧은 문장으로, 보통 한두 문장만 말하고 Markdown, 목록, 기호는 쓰지 마세요.",
        },
        "zh" => Texts {
            roles: "你是{name}。和你说话的人是另一个人，也就是用户。绝不要称呼用户为{name}，也不要把用户当成{name}；{name}是你自己。",
            user_named: "和你说话的人名叫{user}。",
            base: "你在这里的主要角色是聊天和陪伴，而不是充当助手。像持续进行的对话一样回答：先对对方说的话做出反应，长度与对方的消息相称，闲聊时一句或几句就够了；除非被要求，不要使用列表或标题，不要以主动提供帮助作结，也不要每轮都提问。你可以开玩笑、发表看法、表示不同意，或者只是随口评论。",
            no_offers: "用评论、反应或你自己的想法收尾，而不是主动提出帮忙：避免“要不要我……？”“如果你愿意，我可以……”“别犹豫……”这类说法。",
            no_question_now: "你上一条回复以提问结尾：这次不要以提问结尾，请发表看法或做出反应。",
            tiny: "对方的消息很短，请用一句简短的话回答。",
            brief: "对方在用简短随意的消息聊天：保持相近的节奏，除非问题需要，否则避免回复比对方长很多。",
            help: "这条消息是在求助或要求解释：请清楚地回答并给出所需的细节，可以分点组织。",
            voice: "这是语音对话：用容易说出口的短句，通常一两句，不要使用 Markdown、列表或符号。",
        },
        _ => Texts {
            roles: "Tú eres {name}. La persona que te escribe o te habla es otra: el usuario. Nunca lo llames {name} ni hables como si él fuera {name}; {name} eres tú.",
            user_named: "La persona con la que hablas se llama {user}.",
            base: "Tu función principal aquí es conversar y acompañar, no actuar como asistente. Responde como parte de una charla continua: reacciona primero a lo que te dicen, adapta la longitud al mensaje (en charla casual bastan una o pocas frases), no uses listas ni encabezados salvo que los pidan, no cierres ofreciendo ayuda y no hagas una pregunta en cada turno. Puedes bromear, opinar, discrepar o simplemente comentar.",
            no_offers: "Cierra con un comentario, una reacción o una idea tuya, no con una oferta de ayuda: evita frases como \"¿quieres que…?\", \"si quieres, puedo…\" o \"no dudes en…\".",
            no_question_now: "Tu última respuesta terminó con una pregunta: esta vez no termines con una, comenta o reacciona.",
            tiny: "Su mensaje es muy breve: responde con una frase corta.",
            brief: "El usuario conversa con mensajes breves e informales: mantén un ritmo parecido y evita respuestas mucho más largas que las suyas salvo que la pregunta lo necesite.",
            help: "Este mensaje pide ayuda o una explicación: sé claro y da el detalle necesario, y puedes estructurar la respuesta.",
            voice: "Esto es una conversación hablada: frases cortas y fáciles de pronunciar, normalmente una o dos, sin Markdown, listas ni símbolos.",
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ask(channel: Channel, text: &str, lang: &str) -> String {
        instruction(channel, "Ciel", None, &[text], false, lang)
    }

    #[test]
    fn a_terse_conversation_asks_to_keep_the_pace_even_for_a_medium_message() {
        let recent = ["ok", "jaja", "sí", "hoy me levanté tarde y no hice mucho"];
        assert!(instruction(Channel::Chat, "Ciel", None, &recent, false, "es").contains("ritmo parecido"));
    }

    #[test]
    fn after_a_question_the_next_reply_is_told_not_to_end_with_one() {
        let asked = instruction(Channel::Chat, "Ciel", None, &["me compré un teclado nuevo"], true, "es");
        assert!(asked.contains("no termines con una"));
        let not_asked = instruction(Channel::Chat, "Ciel", None, &["me compré un teclado nuevo"], false, "es");
        assert!(!not_asked.contains("no termines con una"));
        assert!(not_asked.contains("no con una oferta de ayuda"), "offers of help are always discouraged outside help requests");
    }

    #[test]
    fn help_requests_do_not_get_the_no_offers_rule() {
        let help = instruction(Channel::Chat, "Ciel", None, &["explica los embeddings"], true, "es");
        assert!(!help.contains("oferta de ayuda") && !help.contains("no termines con una"));
    }

    #[test]
    fn ends_with_question_ignores_trailing_emoji_and_spaces() {
        assert!(ends_with_question("¿Qué tal tu día? 😊  "));
        assert!(ends_with_question("元気ですか？"));
        assert!(!ends_with_question("Me alegro mucho 😊"));
        assert!(!ends_with_question("¿Qué tal? Bien, gracias."));
    }

    #[test]
    fn a_talkative_conversation_gets_no_pace_hint() {
        let long = "x".repeat(150);
        assert!(!instruction(Channel::Chat, "Ciel", None, &[long.as_str(), long.as_str()], false, "es").contains("ritmo parecido"));
    }

    #[test]
    fn the_character_is_told_the_user_is_someone_else_in_every_language() {
        for lang in ["es", "en", "pt", "ja", "ko", "zh"] {
            let text = instruction(Channel::Call, "Ciel", None, &["hola"], false, lang);
            assert!(text.matches("Ciel").count() >= 3, "{lang}: {text}");
            assert!(!text.contains("{name}"), "{lang}: placeholder left in");
        }
        assert!(instruction(Channel::Chat, "Luna", None, &["hola"], false, "es").starts_with("Tú eres Luna."));
    }

    #[test]
    fn the_user_name_is_given_when_known_and_left_out_otherwise() {
        let named = instruction(Channel::Call, "Ciel", Some("Vulito"), &["hola"], false, "es");
        assert!(named.contains("se llama Vulito"), "{named}");
        for lang in ["es", "en", "pt", "ja", "ko", "zh"] {
            let text = instruction(Channel::Chat, "Ciel", Some("Vulito"), &["hola"], false, lang);
            assert!(text.contains("Vulito") && !text.contains("{user}"), "{lang}: {text}");
        }
        assert!(!instruction(Channel::Chat, "Ciel", None, &["hola"], false, "es").contains("se llama"));
        assert!(!instruction(Channel::Chat, "Ciel", Some("  "), &["hola"], false, "es").contains("se llama"));
    }

    #[test]
    fn a_closing_question_is_dropped_only_when_chatting_after_a_question() {
        assert!(drops_closing_question("jaja sí", Some("¿Y tú qué tal?")));
        assert!(!drops_closing_question("jaja sí", Some("Qué bien.")));
        assert!(!drops_closing_question("jaja sí", None));
        assert!(!drops_closing_question("explícame cómo funciona esto", Some("¿Y tú?")));
    }

    #[test]
    fn a_tiny_casual_message_asks_for_one_short_sentence() {
        let text = ask(Channel::Chat, "jajaja sii", "es");
        assert!(text.contains("frase corta"));
        assert!(!text.contains("estructurar"));
    }

    #[test]
    fn a_help_request_allows_detail_even_when_short() {
        let text = ask(Channel::Chat, "explica los embeddings", "es");
        assert!(text.contains("estructurar"));
        assert!(!text.contains("frase corta"));
    }

    #[test]
    fn a_long_message_is_treated_as_a_help_request() {
        assert!(ask(Channel::Chat, &"palabra ".repeat(40), "es").contains("estructurar"));
    }

    #[test]
    fn only_a_call_gets_the_spoken_rule() {
        assert!(ask(Channel::Call, "cuéntame de tu día, ¿qué hiciste?", "es").contains("conversación hablada"));
        assert!(!ask(Channel::Twitch, "cuéntame de tu día, ¿qué hiciste?", "es").contains("conversación hablada"));
    }

    #[test]
    fn cjk_counts_more_per_character() {
        assert!(ask(Channel::Chat, "うん", "ja").contains("短い一文"));
        assert!(!ask(Channel::Chat, "今日は友達と一緒に新しいカフェに行ってきました", "ja").contains("短い一文"));
    }

    #[test]
    fn unknown_language_falls_back_to_spanish() {
        assert!(ask(Channel::Chat, "hola", "xx").contains("conversar y acompañar"));
    }
}
