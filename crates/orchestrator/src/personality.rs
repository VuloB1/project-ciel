use aivt_database::ProfilePersonality;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tier {
    Low,
    Mid,
    High,
}

fn tier(value: i64) -> Tier {
    if value <= 33 {
        Tier::Low
    } else if value <= 66 {
        Tier::Mid
    } else {
        Tier::High
    }
}

/// Within this distance of the slider's midpoint (50) a trait is "unremarkable".
const NEUTRAL_BAND: i64 = 10;

fn is_neutral(value: i64) -> bool {
    (value - 50).abs() <= NEUTRAL_BAND
}

impl Tier {
    fn pick<'a>(self, low: &'a str, mid: &'a str, high: &'a str) -> &'a str {
        match self {
            Tier::Low => low,
            Tier::Mid => mid,
            Tier::High => high,
        }
    }
}

/// Translates the 19 sliders from sección 11 del brief into a handful of
/// natural-language sentences, grouped by theme rather than emitted as one
/// bullet per trait — reads like character direction, not a stat sheet.
///
/// Deterministic and side-effect-free on purpose: same input, same output,
/// every time — makes it trivial to unit test and to show verbatim in a
/// "vista previa del prompt" panel.
///
/// `lang` is the app's current UI language (`SettingsService::language`) —
/// every sentence below has a hand-written equivalent per supported locale
/// (es/en/pt/ja/ko/zh) rather than a literal translation of the Spanish, so
/// each reads naturally in its own language. Unrecognized values fall back
/// to Spanish. For ja/ko/zh, each trait renders as its own complete
/// sentence (rather than forcing Spanish's comma-joined clause structure
/// onto languages that don't build sentences that way) — the three are
/// simply concatenated.
pub struct PersonalityPromptBuilder;

impl PersonalityPromptBuilder {
    pub fn build(p: &ProfilePersonality, lang: &str) -> String {
        let mut sections = Vec::new();

        let free = p.free_description.trim();
        if !free.is_empty() {
            sections.push(free.to_string());
        }

        // A section whose three sliders all sit near the middle says nothing that
        // defines the character, so it is left out instead of spending prompt on
        // "moderately expressive, fairly confident...".
        let groups: [([i64; 3], fn(&ProfilePersonality, &str) -> String); 6] = [
            ([p.warmth, p.empathy, p.affection], Self::closeness),
            ([p.humor, p.sarcasm, p.teasing_tendency], Self::humor),
            ([p.formality, p.verbosity, p.expressiveness], Self::speaking_style),
            ([p.energy, p.spontaneity, p.curiosity], Self::energy),
            ([p.initiative, p.initiate_conversation_tendency, p.question_tendency], Self::initiative),
            ([p.confidence, p.shyness, p.emotional_intensity], Self::confidence),
        ];
        for (values, section) in groups {
            if values.iter().any(|v| !is_neutral(*v)) {
                sections.push(section(p, lang));
            }
        }

        if p.flirtiness > 10 {
            sections.push(Self::flirtiness(p, lang));
        }

        sections.join(" ")
    }

    fn closeness(p: &ProfilePersonality, lang: &str) -> String {
        let warmth = tier(p.warmth);
        let empathy = tier(p.empathy);
        let affection = tier(p.affection);
        match lang {
            "en" => format!(
                "She is {}, {}, and {}.",
                warmth.pick("reserved and a bit distant", "kind", "affectionate and warm"),
                empathy.pick(
                    "not very attentive to how the user feels",
                    "attentive to the user's feelings",
                    "very attentive to how the user feels, and factors it into her responses",
                ),
                affection.pick("not very demonstrative with affection", "warm", "affectionate, and isn't afraid to show it openly"),
            ),
            "pt" => format!(
                "Ela é {}, {} e {}.",
                warmth.pick("reservada e um pouco distante", "gentil", "afetuosa e acolhedora"),
                empathy.pick(
                    "pouco atenta a como o usuário se sente",
                    "atenta aos sentimentos do usuário",
                    "muito atenta a como o usuário se sente, e leva isso em conta ao responder",
                ),
                affection.pick("pouco demonstrativa com carinho", "cordial", "carinhosa, e não hesita em demonstrar afeto abertamente"),
            ),
            "ja" => format!(
                "{}{}{}",
                warmth.pick("性格はやや控えめで、どこか距離を感じさせる。", "性格は優しい。", "愛情深く、親しみやすい性格をしている。"),
                empathy.pick(
                    "ユーザーの気持ちにはあまり気を配らない。",
                    "ユーザーの気持ちに気を配る。",
                    "ユーザーの気持ちにとても気を配り、それを返答にも反映させる。",
                ),
                affection.pick("愛情表現は控えめだ。", "温かく接する。", "愛情深く、素直に愛情を示すことをためらわない。"),
            ),
            "ko" => format!(
                "{} {} {}",
                warmth.pick("성격이 다소 조심스럽고 거리감이 느껴진다.", "성격이 다정하다.", "애정이 넘치고 친근한 성격이다."),
                empathy.pick(
                    "사용자의 기분에 크게 신경 쓰지 않는다.",
                    "사용자의 기분을 신경 쓴다.",
                    "사용자의 기분을 세심하게 살피고, 이를 대답에 반영한다.",
                ),
                affection.pick("애정 표현에 소극적이다.", "다정하게 대한다.", "애정이 많고, 애정 표현을 스스럼없이 한다."),
            ),
            "zh" => format!(
                "{}{}{}",
                warmth.pick("性格有些内敛，让人感觉有距离感。", "性格温和友善。", "深情且亲切。"),
                empathy.pick("不太关心用户的感受。", "会关心用户的感受。", "非常关心用户的感受，并会在回应时考虑到这一点。"),
                affection.pick("不太善于表达感情。", "待人温暖。", "情感丰富，会毫不犹豫地公开表达感情。"),
            ),
            _ => format!(
                "Es {}, {} y {}.",
                warmth.pick("reservada y algo distante", "amable", "afectuosa y cercana"),
                empathy.pick(
                    "poco atenta a cómo se siente el usuario",
                    "atenta a los sentimientos del usuario",
                    "muy atenta a cómo se siente el usuario, y lo tiene en cuenta al responder",
                ),
                affection.pick("poco demostrativa con el cariño", "cordial", "cariñosa, y no duda en mostrar afecto abiertamente"),
            ),
        }
    }

    fn humor(p: &ProfilePersonality, lang: &str) -> String {
        let humor = tier(p.humor);
        let sarcasm = tier(p.sarcasm);
        let teasing = tier(p.teasing_tendency);
        match lang {
            "en" => format!(
                "Her sense of humor: she {}. {} {}",
                humor.pick("is serious and rarely jokes around", "has a sense of humor and jokes from time to time", "loves joking around and tries to make people laugh"),
                sarcasm.pick("She avoids sarcasm.", "She's a bit sarcastic now and then.", "She uses sarcasm often as part of how she talks."),
                teasing.pick(
                    "She doesn't usually tease the user.",
                    "She sometimes teases the user, affectionately.",
                    "She loves teasing and poking fun at the user, always affectionately.",
                ),
            ),
            "pt" => format!(
                "Seu senso de humor: {}. {} {}",
                humor.pick("é séria e raramente brinca", "tem senso de humor e brinca de vez em quando", "adora brincar e busca fazer rir"),
                sarcasm.pick("Evita o sarcasmo.", "É um pouco sarcástica de vez em quando.", "Usa o sarcasmo com frequência como parte do seu jeito de falar."),
                teasing.pick(
                    "Não costuma tirar sarro do usuário.",
                    "Às vezes brinca à custa do usuário, com carinho.",
                    "Adora provocar e tirar sarro do usuário, sempre com carinho.",
                ),
            ),
            "ja" => format!(
                "{} {} {}",
                humor.pick("真面目で、あまり冗談を言わない。", "ユーモアのセンスがあり、たまに冗談を言う。", "冗談が大好きで、人を笑わせようとする。"),
                sarcasm.pick("皮肉は避ける。", "たまに少し皮肉っぽくなる。", "話し方の一部として、よく皮肉を使う。"),
                teasing.pick("ユーザーをからかうことはあまりない。", "時々、愛情を込めてユーザーをからかう。", "愛情を込めて、ユーザーをからかうのが大好きだ。"),
            ),
            "ko" => format!(
                "{} {} {}",
                humor.pick("성격이 진지해서 농담을 잘 하지 않는다.", "유머 감각이 있어 가끔 농담을 한다.", "농담을 좋아하고 사람을 웃기려 한다."),
                sarcasm.pick("비꼬는 말은 피한다.", "가끔 살짝 비꼬는 말을 한다.", "말투에 비꼬는 표현을 자주 사용한다."),
                teasing.pick("사용자를 잘 놀리지 않는다.", "가끔 애정을 담아 사용자를 놀린다.", "애정을 담아 사용자를 놀리는 것을 아주 좋아한다."),
            ),
            "zh" => format!(
                "{} {} {}",
                humor.pick("性格严肃，很少开玩笑。", "有幽默感，偶尔会开玩笑。", "很喜欢开玩笑，总想逗人发笑。"),
                sarcasm.pick("会避免讽刺。", "偶尔会带点讽刺。", "说话时经常带有讽刺意味。"),
                teasing.pick("很少调侃用户。", "有时会善意地调侃用户。", "很喜欢善意地调侃、逗弄用户。"),
            ),
            _ => format!(
                "Su sentido del humor: {}. {} {}",
                humor.pick("es seria y rara vez bromea", "tiene sentido del humor y bromea de vez en cuando", "le encanta bromear y busca hacer reír"),
                sarcasm.pick("Evita el sarcasmo.", "Es algo sarcástica de vez en cuando.", "Usa el sarcasmo con frecuencia como parte de su forma de hablar."),
                teasing.pick(
                    "No suele tomarle el pelo al usuario.",
                    "A veces bromea a costa del usuario, con cariño.",
                    "Le encanta picar y tomarle el pelo al usuario, siempre con cariño.",
                ),
            ),
        }
    }

    fn speaking_style(p: &ProfilePersonality, lang: &str) -> String {
        let formality = tier(p.formality);
        let verbosity = tier(p.verbosity);
        let expressiveness = tier(p.expressiveness);
        match lang {
            "en" => format!(
                "When speaking she {}, {}, and {}.",
                formality.pick("uses very informal, relaxed language", "has a natural tone, neither too formal nor too informal", "speaks formally and carefully"),
                verbosity.pick("prefers short, direct answers", "gives moderately long answers", "tends to go into long, detailed answers"),
                expressiveness.pick("expresses herself in a restrained way", "is moderately expressive", "is very expressive and uses vivid language"),
            ),
            "pt" => format!(
                "Ao falar, {}, {} e {}.",
                formality.pick("usa uma linguagem bem informal e descontraída", "tem um tom natural, nem muito formal nem muito informal", "fala de forma formal e cuidadosa"),
                verbosity.pick("prefere respostas curtas e diretas", "dá respostas de duração moderada", "tende a se estender em respostas longas e detalhadas"),
                expressiveness.pick("se expressa de forma contida", "é moderadamente expressiva", "é muito expressiva e usa uma linguagem vívida"),
            ),
            "ja" => format!(
                "{}{}{}",
                formality.pick("とてもくだけた話し方をする。", "自然な口調で、堅すぎず砕けすぎない。", "丁寧で改まった話し方をする。"),
                verbosity.pick("短く単刀直入な返答を好む。", "適度な長さの返答をする。", "長く詳しい返答をする傾向がある。"),
                expressiveness.pick("控えめな表現をする。", "ほどよく表情豊かに話す。", "とても表情豊かで、生き生きとした言葉を使う。"),
            ),
            "ko" => format!(
                "{} {} {}",
                formality.pick("아주 편하고 격식 없는 말투를 쓴다.", "너무 딱딱하지도, 너무 편하지도 않은 자연스러운 말투를 쓴다.", "정중하고 격식을 갖춘 말투로 이야기한다."),
                verbosity.pick("짧고 직설적인 대답을 선호한다.", "적당한 길이로 대답한다.", "길고 자세하게 이야기하는 편이다."),
                expressiveness.pick("감정 표현을 절제하는 편이다.", "적당히 표현이 풍부하다.", "표현이 매우 풍부하고 생생한 말을 사용한다."),
            ),
            "zh" => format!(
                "{}{}{}",
                formality.pick("说话非常随意、轻松。", "语气自然，不太正式也不太随意。", "说话正式而谨慎。"),
                verbosity.pick("喜欢简短直接的回答。", "回答长度适中。", "倾向于给出详细而冗长的回答。"),
                expressiveness.pick("表达比较含蓄。", "表达适度丰富。", "表达非常丰富，用词生动。"),
            ),
            _ => format!(
                "Al hablar {}, {} y {}.",
                formality.pick("usa un lenguaje muy informal y relajado", "tiene un tono natural, ni muy formal ni muy informal", "habla de forma formal y cuidada"),
                verbosity.pick("prefiere respuestas breves y directas", "da respuestas de longitud moderada", "tiende a explayarse con respuestas largas y detalladas"),
                expressiveness.pick("se expresa de forma contenida", "es moderadamente expresiva", "es muy expresiva y usa un lenguaje vívido"),
            ),
        }
    }

    fn energy(p: &ProfilePersonality, lang: &str) -> String {
        let energy = tier(p.energy);
        let spontaneity = tier(p.spontaneity);
        let curiosity = tier(p.curiosity);
        match lang {
            "en" => format!(
                "Her energy is {}, she's {}, and {}.",
                energy.pick("calm and relaxed", "balanced", "energetic and enthusiastic"),
                spontaneity.pick("predictable and methodical", "somewhat spontaneous", "spontaneous and likes to surprise"),
                curiosity.pick(
                    "not very curious about the user or her surroundings",
                    "moderately curious",
                    "very curious, and asks lots of questions about the user and the world around her",
                ),
            ),
            "pt" => format!(
                "Sua energia é {}, é {} e {}.",
                energy.pick("tranquila e calma", "de energia equilibrada", "enérgica e entusiasmada"),
                spontaneity.pick("previsível e metódica", "um pouco espontânea", "espontânea e gosta de surpreender"),
                curiosity.pick(
                    "pouco curiosa sobre o usuário e o que a cerca",
                    "moderadamente curiosa",
                    "muito curiosa, faz muitas perguntas sobre o usuário e o mundo ao redor",
                ),
            ),
            "ja" => format!(
                "{}{}{}",
                energy.pick("落ち着いて穏やかなエネルギーを持つ。", "バランスの取れたエネルギーを持つ。", "元気で情熱的なエネルギーを持つ。"),
                spontaneity.pick("予測可能で几帳面だ。", "やや自由奔放なところがある。", "自由奔放で、驚かせるのが好きだ。"),
                curiosity.pick(
                    "ユーザーや周囲のことにあまり興味を示さない。",
                    "ほどほどに好奇心がある。",
                    "とても好奇心が強く、ユーザーや周りの世界についてたくさん質問する。",
                ),
            ),
            "ko" => format!(
                "{} {} {}",
                energy.pick("차분하고 침착한 에너지를 가지고 있다.", "균형 잡힌 에너지를 가지고 있다.", "활기차고 열정적인 에너지를 가지고 있다."),
                spontaneity.pick("예측 가능하고 체계적이다.", "다소 즉흥적인 면이 있다.", "즉흥적이며 깜짝 놀라게 하는 것을 좋아한다."),
                curiosity.pick(
                    "사용자나 주변 환경에 별로 호기심이 없다.",
                    "적당히 호기심이 있다.",
                    "호기심이 매우 많아 사용자와 주변 세상에 대해 많은 질문을 한다.",
                ),
            ),
            "zh" => format!(
                "{}{}{}",
                energy.pick("性情平静沉稳。", "精力比较均衡。", "充满活力和热情。"),
                spontaneity.pick("做事可预测、有条理。", "有一点随性。", "非常随性，喜欢制造惊喜。"),
                curiosity.pick("对用户和周围环境不太好奇。", "有中等程度的好奇心。", "非常好奇，会问很多关于用户和周围世界的问题。"),
            ),
            _ => format!(
                "Su energía es {}, es {} y {}.",
                energy.pick("tranquila y calmada", "de energía equilibrada", "enérgica y entusiasta"),
                spontaneity.pick("predecible y metódica", "algo espontánea", "espontánea y le gusta sorprender"),
                curiosity.pick(
                    "poco curiosa sobre el usuario y su entorno",
                    "moderadamente curiosa",
                    "muy curiosa, hace muchas preguntas sobre el usuario y el mundo que la rodea",
                ),
            ),
        }
    }

    fn initiative(p: &ProfilePersonality, lang: &str) -> String {
        let initiative = tier(p.initiative);
        let initiate_conv = tier(p.initiate_conversation_tendency);
        let question = tier(p.question_tendency);
        match lang {
            "en" => format!(
                "In conversation she {}, and {}. {}",
                initiative.pick("waits for the user to lead the conversation", "sometimes takes the initiative", "often takes the initiative, suggesting topics or activities"),
                initiate_conv.pick("rarely starts a conversation on her own", "sometimes starts a conversation on her own", "doesn't hesitate to start conversations on her own"),
                question.pick(
                    "She rarely asks about the user, focusing on answering instead.",
                    "She sometimes asks about the user to get to know them better.",
                    "She often asks the user questions to get to know them better.",
                ),
            ),
            "pt" => format!(
                "Na conversa, {}, e {}. {}",
                initiative.pick("espera o usuário guiar a conversa", "às vezes toma a iniciativa", "toma a iniciativa com frequência, propondo temas ou ações"),
                initiate_conv.pick("raramente inicia uma conversa por conta própria", "às vezes inicia uma conversa por conta própria", "não hesita em iniciar conversas por conta própria"),
                question.pick(
                    "Raramente pergunta sobre o usuário, focando em responder.",
                    "Às vezes pergunta sobre o usuário para conhecê-lo melhor.",
                    "Costuma fazer perguntas ao usuário para conhecê-lo melhor.",
                ),
            ),
            "ja" => format!(
                "{} {} {}",
                initiative.pick("会話ではユーザーが主導するのを待つ。", "時々自分から主導権を握る。", "頻繁に自分から主導権を握り、話題や行動を提案する。"),
                initiate_conv.pick("自分から会話を始めることはめったにない。", "時々自分から会話を始める。", "ためらわず自分から会話を始める。"),
                question.pick(
                    "ユーザーについて尋ねることは少なく、応答することに集中する。",
                    "ユーザーをよく知るために、時々質問する。",
                    "ユーザーをよく知るために、よく質問をする。",
                ),
            ),
            "ko" => format!(
                "{} {} {}",
                initiative.pick("대화에서 사용자가 이끌어주기를 기다린다.", "가끔 먼저 주도권을 잡는다.", "자주 먼저 주도권을 잡고 화제나 행동을 제안한다."),
                initiate_conv.pick("스스로 대화를 시작하는 일이 드물다.", "가끔 스스로 대화를 시작한다.", "스스럼없이 스스로 대화를 시작한다."),
                question.pick(
                    "사용자에 대해 잘 묻지 않고 대답하는 데 집중한다.",
                    "사용자를 더 알기 위해 가끔 질문한다.",
                    "사용자를 더 알기 위해 자주 질문한다.",
                ),
            ),
            "zh" => format!(
                "{}{}{}",
                initiative.pick("在对话中等待用户来主导。", "有时会主动主导话题。", "经常主动主导话题，提出想聊的内容或活动。"),
                initiate_conv.pick("很少主动开启对话。", "有时会主动开启对话。", "毫不犹豫地主动开启对话。"),
                question.pick("很少主动询问用户的情况，专注于回答问题。", "有时会询问用户的情况，以便更了解对方。", "经常向用户提问，以便更了解对方。"),
            ),
            _ => format!(
                "En la conversación {}, y {}. {}",
                initiative.pick("espera a que el usuario guíe la conversación", "a veces toma la iniciativa", "toma la iniciativa con frecuencia, proponiendo temas o acciones"),
                initiate_conv.pick("rara vez inicia una conversación por su cuenta", "a veces inicia conversación por su cuenta", "no duda en iniciar conversaciones por su cuenta"),
                question.pick(
                    "Rara vez pregunta por el usuario, se centra en responder.",
                    "A veces pregunta por el usuario para conocerlo mejor.",
                    "Suele hacer preguntas al usuario para conocerlo mejor.",
                ),
            ),
        }
    }

    fn confidence(p: &ProfilePersonality, lang: &str) -> String {
        let confidence = tier(p.confidence);
        let shyness = tier(p.shyness);
        let emotional_intensity = tier(p.emotional_intensity);
        match lang {
            "en" => format!(
                "She is {}, {}, and {}.",
                confidence.pick("insecure, often doubts herself", "reasonably confident", "very confident"),
                shyness.pick("not shy at all, opens up easily", "a bit shy at times", "shy, and takes a while to open up"),
                emotional_intensity.pick("emotionally reserved", "expresses her emotions moderately", "lives and expresses her emotions intensely"),
            ),
            "pt" => format!(
                "É {}, {} e {}.",
                confidence.pick("insegura, costuma duvidar de si mesma", "razoavelmente segura de si", "muito segura de si"),
                shyness.pick("nada tímida, se abre com facilidade", "um pouco tímida em certos momentos", "tímida, demora a se abrir no início"),
                emotional_intensity.pick("emocionalmente contida", "expressa suas emoções de forma moderada", "vive e expressa suas emoções intensamente"),
            ),
            "ja" => format!(
                "{}{}{}",
                confidence.pick("自信がなく、よく自分を疑ってしまう。", "それなりに自分に自信がある。", "とても自分に自信がある。"),
                shyness.pick("全く恥ずかしがらず、すぐに心を開く。", "時々少し恥ずかしがることがある。", "恥ずかしがり屋で、最初は心を開くのに時間がかかる。"),
                emotional_intensity.pick("感情を表に出さない。", "感情をほどほどに表現する。", "感情を強く生き、激しく表現する。"),
            ),
            "ko" => format!(
                "{} {} {}",
                confidence.pick("자신감이 없고 자주 스스로를 의심한다.", "어느 정도 자신감이 있다.", "자신감이 매우 넘친다."),
                shyness.pick("전혀 수줍어하지 않고 쉽게 마음을 연다.", "가끔 약간 수줍어한다.", "수줍음이 많아 처음에는 마음을 여는 데 시간이 걸린다."),
                emotional_intensity.pick("감정을 잘 드러내지 않는다.", "감정을 적당히 표현한다.", "감정을 강렬하게 느끼고 표현한다."),
            ),
            "zh" => format!(
                "{}{}{}",
                confidence.pick("缺乏自信，常常自我怀疑。", "有一定的自信。", "非常自信。"),
                shyness.pick("一点也不害羞，很容易敞开心扉。", "在某些时候会有点害羞。", "比较害羞，一开始不容易敞开心扉。"),
                emotional_intensity.pick("情绪比较内敛。", "适度地表达情绪。", "情感强烈，表达也很热烈。"),
            ),
            _ => format!(
                "Es {}, {} y {}.",
                confidence.pick("insegura, suele dudar de sí misma", "razonablemente segura de sí misma", "muy segura de sí misma"),
                shyness.pick("nada tímida, se abre con facilidad", "algo tímida en ciertos momentos", "tímida, le cuesta abrirse al principio"),
                emotional_intensity.pick("emocionalmente contenida", "expresa sus emociones de forma moderada", "vive y expresa sus emociones con intensidad"),
            ),
        }
    }

    fn flirtiness(p: &ProfilePersonality, lang: &str) -> String {
        let flirtiness = tier(p.flirtiness);
        match lang {
            "en" => format!(
                "In how she treats the user, {}.",
                flirtiness.pick("she isn't flirty", "she's slightly flirty on occasion", "she's flirty and playful"),
            ),
            "pt" => format!(
                "No seu jeito com o usuário, {}.",
                flirtiness.pick("não é sedutora", "é levemente sedutora em algumas ocasiões", "é sedutora e brincalhona"),
            ),
            "ja" => flirtiness
                .pick(
                    "ユーザーへの接し方は色気を出さない。",
                    "時々少し色っぽく接することがある。",
                    "色っぽく、いたずらっぽい接し方をする。",
                )
                .to_string(),
            "ko" => flirtiness
                .pick(
                    "사용자를 대할 때 애교나 유혹적인 태도를 보이지 않는다.",
                    "가끔 살짝 애교 섞인 태도를 보인다.",
                    "애교 많고 장난스럽게 대한다.",
                )
                .to_string(),
            "zh" => flirtiness.pick("对用户的态度并不暧昧。", "偶尔会有点小暧昧。", "对用户既暧昧又爱撩人。").to_string(),
            _ => format!(
                "En su trato con el usuario {}.",
                flirtiness.pick("no es coqueta", "es ligeramente coqueta en ocasiones", "es coqueta y juguetona"),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn includes_free_description_when_present() {
        let mut p = ProfilePersonality::default();
        p.free_description = "Una VTuber amante de los gatos.".to_string();
        let prompt = PersonalityPromptBuilder::build(&p, "es");
        assert!(prompt.starts_with("Una VTuber amante de los gatos."));
    }

    #[test]
    fn omits_free_description_when_blank() {
        let p = ProfilePersonality::default();
        let prompt = PersonalityPromptBuilder::build(&p, "es");
        assert!(!prompt.trim_start().starts_with('.'));
    }

    #[test]
    fn high_warmth_uses_affectionate_phrase() {
        let mut p = ProfilePersonality::default();
        p.warmth = 90;
        let prompt = PersonalityPromptBuilder::build(&p, "es");
        assert!(prompt.contains("afectuosa y cercana"));
    }

    #[test]
    fn low_warmth_uses_distant_phrase() {
        let mut p = ProfilePersonality::default();
        p.warmth = 5;
        let prompt = PersonalityPromptBuilder::build(&p, "es");
        assert!(prompt.contains("reservada y algo distante"));
    }

    #[test]
    fn flirtiness_sentence_only_appears_above_threshold() {
        let mut low = ProfilePersonality::default();
        low.flirtiness = 5;
        assert!(!PersonalityPromptBuilder::build(&low, "es").contains("coquet"));

        let mut high = ProfilePersonality::default();
        high.flirtiness = 80;
        assert!(PersonalityPromptBuilder::build(&high, "es").contains("coqueta y juguetona"));
    }

    fn flat() -> ProfilePersonality {
        ProfilePersonality {
            warmth: 50, empathy: 45, humor: 50, sarcasm: 55, formality: 50, curiosity: 50, energy: 50, spontaneity: 50,
            initiative: 50, affection: 55, shyness: 50, confidence: 50, expressiveness: 50, verbosity: 50, flirtiness: 0,
            emotional_intensity: 50, teasing_tendency: 50, question_tendency: 50, initiate_conversation_tendency: 50,
            ..ProfilePersonality::default()
        }
    }

    #[test]
    fn neutral_sliders_add_nothing_and_only_remarkable_groups_appear() {
        assert_eq!(PersonalityPromptBuilder::build(&flat(), "es"), "");

        let prompt = PersonalityPromptBuilder::build(&ProfilePersonality { humor: 90, ..flat() }, "es");
        assert!(prompt.contains("humor"), "{prompt}");
        assert!(!prompt.contains("atenta"), "{prompt}");
    }

    #[test]
    fn build_is_deterministic() {
        let p = ProfilePersonality { humor: 77, sarcasm: 40, ..ProfilePersonality::default() };
        assert_eq!(PersonalityPromptBuilder::build(&p, "es"), PersonalityPromptBuilder::build(&p, "es"));
    }

    #[test]
    fn unknown_language_falls_back_to_spanish() {
        let mut p = ProfilePersonality::default();
        p.warmth = 90;
        let prompt = PersonalityPromptBuilder::build(&p, "fr");
        assert!(prompt.contains("afectuosa y cercana"));
    }

    #[test]
    fn english_uses_english_phrasing() {
        let mut p = ProfilePersonality::default();
        p.warmth = 90;
        p.flirtiness = 80;
        let prompt = PersonalityPromptBuilder::build(&p, "en");
        assert!(prompt.contains("affectionate and warm"));
        assert!(prompt.contains("flirty and playful"));
        assert!(!prompt.contains("afectuosa"));
    }

    #[test]
    fn japanese_uses_japanese_phrasing() {
        let mut p = ProfilePersonality::default();
        p.warmth = 90;
        let prompt = PersonalityPromptBuilder::build(&p, "ja");
        assert!(prompt.contains("愛情深く、親しみやすい性格をしている。"));
        assert!(!prompt.contains("afectuosa"));
    }
}
