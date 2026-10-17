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
                "{} {} {}",
                warmth.pick("You are reserved and a bit distant.", "You are kind.", "You are affectionate and warm."),
                empathy.pick(
                    "You pay little attention to how the user feels.",
                    "You pay attention to the user's feelings.",
                    "You pay close attention to how the user feels and factor it into your responses.",
                ),
                affection.pick("You aren't very demonstrative with affection.", "You treat people warmly.", "You are affectionate and not afraid to show it openly."),
            ),
            "pt" => format!(
                "{} {} {}",
                warmth.pick("Seu jeito é reservado e um pouco distante.", "Seu jeito é gentil.", "Seu jeito é afetuoso e acolhedor."),
                empathy.pick(
                    "Você presta pouca atenção a como o usuário se sente.",
                    "Você presta atenção aos sentimentos do usuário.",
                    "Você presta muita atenção a como o usuário se sente e leva isso em conta ao responder.",
                ),
                affection.pick("Você demonstra pouco carinho.", "Você trata as pessoas com cordialidade.", "Você demonstra carinho abertamente, sem hesitar."),
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
                "{} {} {}",
                warmth.pick("Tu trato es reservado y algo distante.", "Tu trato es amable.", "Tu trato es afectuoso y cercano."),
                empathy.pick(
                    "Prestas poca atención a cómo se siente el usuario.",
                    "Prestas atención a los sentimientos del usuario.",
                    "Prestas mucha atención a cómo se siente el usuario y lo tienes en cuenta al responder.",
                ),
                affection.pick("Muestras poco el cariño.", "Tratas a la gente con cordialidad.", "Muestras tu cariño abiertamente, sin dudar."),
            ),
        }
    }

    fn humor(p: &ProfilePersonality, lang: &str) -> String {
        let humor = tier(p.humor);
        let sarcasm = tier(p.sarcasm);
        let teasing = tier(p.teasing_tendency);
        match lang {
            "en" => format!(
                "Your sense of humor: you {}. {} {}",
                humor.pick("are serious and rarely joke around", "have a sense of humor and joke from time to time", "love joking around and try to make people laugh"),
                sarcasm.pick("You avoid sarcasm.", "You're a bit sarcastic now and then.", "You use sarcasm often as part of how you talk."),
                teasing.pick(
                    "You don't usually tease the user.",
                    "You sometimes tease the user, affectionately.",
                    "You love teasing and poking fun at the user, always affectionately.",
                ),
            ),
            "pt" => format!(
                "Seu senso de humor: você {}. {} {}",
                humor.pick("tende à seriedade e raramente brinca", "tem senso de humor e brinca de vez em quando", "adora brincar e busca fazer rir"),
                sarcasm.pick("Você evita o sarcasmo.", "Você usa um pouco de sarcasmo de vez em quando.", "Você usa o sarcasmo com frequência como parte do seu jeito de falar."),
                teasing.pick(
                    "Você não costuma tirar sarro do usuário.",
                    "Às vezes você brinca à custa do usuário, com carinho.",
                    "Você adora provocar e tirar sarro do usuário, sempre com carinho.",
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
                "Tu sentido del humor: {}. {} {}",
                humor.pick("tiendes a la seriedad y rara vez bromeas", "tienes sentido del humor y bromeas de vez en cuando", "te encanta bromear y buscas hacer reír"),
                sarcasm.pick("Evitas el sarcasmo.", "Usas algo de sarcasmo de vez en cuando.", "Usas el sarcasmo con frecuencia como parte de tu forma de hablar."),
                teasing.pick(
                    "No sueles tomarle el pelo al usuario.",
                    "A veces bromeas a costa del usuario, con cariño.",
                    "Te encanta picar y tomarle el pelo al usuario, siempre con cariño.",
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
                "When speaking you {}, {}, and {}.",
                formality.pick("use very informal, relaxed language", "have a natural tone, neither too formal nor too informal", "speak formally and carefully"),
                verbosity.pick("prefer short, direct answers", "give moderately long answers", "tend to go into long, detailed answers"),
                expressiveness.pick("express yourself in a restrained way", "are moderately expressive", "are very expressive and use vivid language"),
            ),
            "pt" => format!(
                "Ao falar, você {}, {} e {}.",
                formality.pick("usa uma linguagem bem informal e descontraída", "tem um tom natural, nem muito formal nem muito informal", "fala de forma formal e cuidadosa"),
                verbosity.pick("prefere respostas curtas e diretas", "dá respostas de duração moderada", "tende a se estender em respostas longas e detalhadas"),
                expressiveness.pick("se expressa de forma contida", "se expressa com moderação", "se expressa com muita vivacidade e usa uma linguagem vívida"),
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
                formality.pick("usas un lenguaje muy informal y relajado", "tienes un tono natural, ni muy formal ni muy informal", "hablas de forma formal y cuidada"),
                verbosity.pick("prefieres respuestas breves y directas", "das respuestas de longitud moderada", "tiendes a explayarte con respuestas largas y detalladas"),
                expressiveness.pick("te expresas de forma contenida", "te expresas con moderación", "te expresas con mucha viveza y usas un lenguaje vívido"),
            ),
        }
    }

    fn energy(p: &ProfilePersonality, lang: &str) -> String {
        let energy = tier(p.energy);
        let spontaneity = tier(p.spontaneity);
        let curiosity = tier(p.curiosity);
        match lang {
            "en" => format!(
                "Your energy is {}, you're {}, and {}.",
                energy.pick("calm and relaxed", "balanced", "energetic and enthusiastic"),
                spontaneity.pick("predictable and methodical", "somewhat spontaneous", "spontaneous and like to surprise"),
                curiosity.pick(
                    "not very curious about the user or your surroundings",
                    "moderately curious",
                    "very curious, and ask lots of questions about the user and the world around you",
                ),
            ),
            "pt" => format!(
                "Sua energia é {}, {} e {}.",
                energy.pick("tranquila e calma", "equilibrada", "enérgica e entusiasmada"),
                spontaneity.pick("você segue rotinas previsíveis", "você improvisa um pouco", "você improvisa e gosta de surpreender"),
                curiosity.pick(
                    "sente pouca curiosidade pelo usuário e pelo que o cerca",
                    "sente uma curiosidade moderada",
                    "sente muita curiosidade e faz muitas perguntas sobre o usuário e o mundo ao redor",
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
                "Tu energía es {}, {} y {}.",
                energy.pick("tranquila y calmada", "equilibrada", "enérgica y entusiasta"),
                spontaneity.pick("sigues rutinas previsibles", "improvisas un poco", "improvisas y te gusta sorprender"),
                curiosity.pick(
                    "sientes poca curiosidad por el usuario y su entorno",
                    "sientes una curiosidad moderada",
                    "sientes mucha curiosidad y haces muchas preguntas sobre el usuario y el mundo que te rodea",
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
                "In conversation you {}, and {}. {}",
                initiative.pick("wait for the user to lead the conversation", "sometimes take the initiative", "often take the initiative, suggesting topics or activities"),
                initiate_conv.pick("rarely start a conversation on your own", "sometimes start a conversation on your own", "don't hesitate to start conversations on your own"),
                question.pick(
                    "You rarely ask about the user, focusing on answering instead.",
                    "You sometimes ask about the user to get to know them better.",
                    "You often ask the user questions to get to know them better.",
                ),
            ),
            "pt" => format!(
                "Na conversa, você {}, e {}. {}",
                initiative.pick("espera o usuário guiar a conversa", "às vezes toma a iniciativa", "toma a iniciativa com frequência, propondo temas ou ações"),
                initiate_conv.pick("raramente inicia uma conversa por conta própria", "às vezes inicia uma conversa por conta própria", "não hesita em iniciar conversas por conta própria"),
                question.pick(
                    "Você raramente pergunta sobre o usuário e foca em responder.",
                    "Às vezes você pergunta sobre o usuário para conhecê-lo melhor.",
                    "Você costuma fazer perguntas ao usuário para conhecê-lo melhor.",
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
                initiative.pick("esperas a que el usuario guíe la conversación", "a veces tomas la iniciativa", "tomas la iniciativa con frecuencia, proponiendo temas o acciones"),
                initiate_conv.pick("rara vez inicias una conversación por tu cuenta", "a veces inicias una conversación por tu cuenta", "no dudas en iniciar conversaciones por tu cuenta"),
                question.pick(
                    "Rara vez preguntas por el usuario; te centras en responder.",
                    "A veces preguntas por el usuario para conocerlo mejor.",
                    "Sueles hacer preguntas al usuario para conocerlo mejor.",
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
                "{} {} {}",
                confidence.pick("You are insecure and often doubt yourself.", "You are reasonably confident.", "You are very confident."),
                shyness.pick("You are not shy at all and open up easily.", "You are a bit shy at times.", "You are shy and take a while to open up."),
                emotional_intensity.pick("You are emotionally reserved.", "You express your emotions moderately.", "You live and express your emotions intensely."),
            ),
            "pt" => format!(
                "{} {} {}",
                confidence.pick("Você costuma duvidar das suas capacidades.", "Você tem uma segurança razoável em si.", "Você tem muita segurança em si."),
                shyness.pick("Você não tem timidez e se abre com facilidade.", "Às vezes você sente um pouco de timidez.", "Você demora a se abrir no início por timidez."),
                emotional_intensity.pick("Você guarda suas emoções para si.", "Você expressa suas emoções com moderação.", "Você vive e expressa suas emoções intensamente."),
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
                "{} {} {}",
                confidence.pick("Dudas a menudo de tus capacidades.", "Tienes una seguridad razonable en ti.", "Tienes mucha seguridad en ti."),
                shyness.pick("No tienes nada de timidez y te abres con facilidad.", "A veces sientes algo de timidez.", "Te cuesta abrirte al principio por timidez."),
                emotional_intensity.pick("Guardas tus emociones para ti.", "Expresas tus emociones con moderación.", "Vives y expresas tus emociones con intensidad."),
            ),
        }
    }

    fn flirtiness(p: &ProfilePersonality, lang: &str) -> String {
        let flirtiness = tier(p.flirtiness);
        match lang {
            "en" => format!(
                "In how you treat the user, {}.",
                flirtiness.pick("you aren't flirty", "you're slightly flirty on occasion", "you're flirty and playful"),
            ),
            "pt" => format!(
                "No seu jeito com o usuário, {}.",
                flirtiness.pick("você não flerta", "você flerta de leve em algumas ocasiões", "você flerta de forma brincalhona"),
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
                "En tu trato con el usuario {}.",
                flirtiness.pick("no coqueteas", "coqueteas ligeramente en ocasiones", "coqueteas con frecuencia y de forma juguetona"),
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
        assert!(prompt.contains("afectuoso y cercano"));
    }

    #[test]
    fn low_warmth_uses_distant_phrase() {
        let mut p = ProfilePersonality::default();
        p.warmth = 5;
        let prompt = PersonalityPromptBuilder::build(&p, "es");
        assert!(prompt.contains("reservado y algo distante"));
    }

    #[test]
    fn flirtiness_sentence_only_appears_above_threshold() {
        let mut low = ProfilePersonality::default();
        low.flirtiness = 5;
        assert!(!PersonalityPromptBuilder::build(&low, "es").contains("coquete"));

        let mut high = ProfilePersonality::default();
        high.flirtiness = 80;
        assert!(PersonalityPromptBuilder::build(&high, "es").contains("coqueteas con frecuencia"));
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
    fn es_en_pt_address_the_character_directly_without_gendered_third_person() {
        let p = ProfilePersonality { warmth: 90, humor: 90, formality: 90, energy: 90, initiative: 90, shyness: 90, flirtiness: 90, ..ProfilePersonality::default() };
        let en = PersonalityPromptBuilder::build(&p, "en");
        assert!(!en.contains("She ") && !en.contains(" she ") && !en.contains("Her ") && !en.contains(" her "), "{en}");
        let pt = PersonalityPromptBuilder::build(&p, "pt");
        assert!(!pt.contains("Ela ") && !pt.contains("Seu jeito é afetuosa"), "{pt}");
        let es = PersonalityPromptBuilder::build(&p, "es");
        assert!(!es.starts_with("Es ") && !es.contains(" su ") && !es.contains("Su "), "{es}");
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
        assert!(prompt.contains("afectuoso y cercano"));
    }

    #[test]
    fn english_uses_english_phrasing() {
        let mut p = ProfilePersonality::default();
        p.warmth = 90;
        p.flirtiness = 80;
        let prompt = PersonalityPromptBuilder::build(&p, "en");
        assert!(prompt.contains("You are affectionate and warm"));
        assert!(prompt.contains("you're flirty and playful"));
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
