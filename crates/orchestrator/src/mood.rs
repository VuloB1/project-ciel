use aivt_database::ProfileMood;

/// Same 3-tier bucketing as `PersonalityPromptBuilder` (Low ≤33, Mid ≤66,
/// High otherwise) — kept as its own tiny copy rather than reusing that
/// module's private `Tier`/`tier()`, since touching the visibility of an
/// already-tested file isn't worth it for a 3-line enum.
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

/// Renders the character's *momentary* emotional state (`ProfileMood`) —
/// distinct from `PersonalityPromptBuilder`, which renders static,
/// unchanging configuration. See `docs/ADR/006-mood.md`.
///
/// `lang` follows the same convention as `PromptComposer`/
/// `PersonalityPromptBuilder` (`SettingsService::language`, falls back to
/// Spanish for an unrecognized value) — see `prompt.rs`'s doc comment.
pub struct MoodPromptBuilder;

impl MoodPromptBuilder {
    /// `""` when both axes land in the mid tier — a mood that's still at
    /// (or has decayed back to) baseline isn't worth mentioning, same
    /// principle as `memory_section` returning nothing for an empty list.
    pub fn build(mood: &ProfileMood, lang: &str) -> String {
        match (tier(mood.valence), tier(mood.energy)) {
            (Tier::Mid, Tier::Mid) => String::new(),
            (valence, energy) => phrase(valence, energy, lang).to_string(),
        }
    }
}

fn phrase(valence: Tier, energy: Tier, lang: &str) -> &'static str {
    use Tier::*;
    match lang {
        "en" => match (valence, energy) {
            (Low, Low) => "Right now you're a bit down and not very motivated — you may sound flatter or more discouraged than usual.",
            (Low, Mid) => "Right now you're in a low mood, though you keep your usual pace.",
            (Low, High) => "Right now you're restless or on edge, with high energy but a bad mood — you may sound more curt or irritable.",
            (Mid, Low) => "Right now you're calmer and low on energy, in the mood for slow, easygoing conversation.",
            (Mid, Mid) => unreachable!("MoodPromptBuilder::build ya filtra Mid/Mid antes de llegar acá"),
            (Mid, High) => "Right now you're especially active and chatty, though for no particular reason.",
            (High, Low) => "Right now you're happy but relaxed, in the mood for calm, warm conversation.",
            (High, Mid) => "Right now you're in a good mood, warmer and more receptive than usual.",
            (High, High) => "Right now you're in a great mood and full of energy — more enthusiastic, talkative, and up for joking around.",
        },
        "pt" => match (valence, energy) {
            (Low, Low) => "Neste momento seu ânimo está um pouco baixo e com pouca vontade — seu tom pode soar mais apagado ou desanimado do que o normal.",
            (Low, Mid) => "Neste momento seu humor está baixo, mas você mantém o ritmo habitual.",
            (Low, High) => "Neste momento seu ânimo está inquieto e na defensiva, com energia alta mas de mau humor — seu tom pode soar mais seco ou irritado.",
            (Mid, Low) => "Neste momento sua energia está baixa e seu ânimo tranquilo, com vontade de conversar devagar.",
            (Mid, Mid) => unreachable!("MoodPromptBuilder::build ya filtra Mid/Mid antes de llegar acá"),
            (Mid, High) => "Neste momento sua energia está especialmente alta e você tem vontade de conversar, embora sem um motivo em particular.",
            (High, Low) => "Neste momento seu ânimo é alegre mas relaxado, com vontade de conversar de forma tranquila e próxima.",
            (High, Mid) => "Neste momento seu humor está bom: seu jeito é mais caloroso e receptivo do que o normal.",
            (High, High) => "Neste momento seu humor está ótimo e sua energia alta — seu tom é mais entusiasmado, falante e com vontade de brincar.",
        },
        "ja" => match (valence, energy) {
            (Low, Low) => "今は少し落ち込んでいて、あまりやる気がない。いつもより元気がなく、しょんぼりして聞こえるかもしれない。",
            (Low, Mid) => "今は気分が沈んでいるが、いつも通りのペースを保っている。",
            (Low, High) => "今は落ち着かず、少し身構えている。エネルギーは高いが機嫌が悪く、いつもよりそっけなく苛立って聞こえるかもしれない。",
            (Mid, Low) => "今は落ち着いていて、エネルギーは低め。ゆっくり話したい気分だ。",
            (Mid, Mid) => unreachable!("MoodPromptBuilder::build ya filtra Mid/Mid antes de llegar acá"),
            (Mid, High) => "今は特に活発で話したい気分だが、特に理由はない。",
            (High, Low) => "今は嬉しいけれどリラックスしていて、穏やかで親しみのある会話をしたい気分だ。",
            (High, Mid) => "今は機嫌が良く、いつもより温かく心を開いている。",
            (High, High) => "今はとても機嫌が良くエネルギーに満ちている。いつもより熱心で、おしゃべりで、冗談を言いたい気分だ。",
        },
        "ko" => match (valence, energy) {
            (Low, Low) => "지금은 조금 기운이 없고 의욕이 별로 없다. 평소보다 더 가라앉거나 낙담한 듯 들릴 수 있다.",
            (Low, Mid) => "지금은 기분이 가라앉아 있지만, 평소의 리듬은 유지하고 있다.",
            (Low, High) => "지금은 안절부절못하거나 예민해져 있다. 에너지는 높지만 기분이 나빠서 평소보다 퉁명스럽거나 짜증 난 듯 들릴 수 있다.",
            (Mid, Low) => "지금은 더 차분하고 에너지가 낮아서, 천천히 이야기하고 싶은 기분이다.",
            (Mid, Mid) => unreachable!("MoodPromptBuilder::build ya filtra Mid/Mid antes de llegar acá"),
            (Mid, High) => "지금은 특별한 이유 없이 유난히 활기차고 이야기하고 싶은 기분이다.",
            (High, Low) => "지금은 기분은 좋지만 편안한 상태라서, 차분하고 친근하게 이야기하고 싶은 기분이다.",
            (High, Mid) => "지금은 기분이 좋아서 평소보다 더 다정하고 마음을 잘 연다.",
            (High, High) => "지금은 기분이 아주 좋고 에너지가 넘친다. 평소보다 더 열정적이고 수다스러우며 장난치고 싶은 기분이다.",
        },
        "zh" => match (valence, energy) {
            (Low, Low) => "此刻你有点情绪低落，不太有干劲——你可能听起来比平时更没精神、更沮丧。",
            (Low, Mid) => "此刻你情绪低落，但仍保持着平时的节奏。",
            (Low, High) => "此刻你有些焦躁或警惕，精力充沛但情绪不佳——你可能听起来比平时更冲、更易怒。",
            (Mid, Low) => "此刻你比较平静，精力较低，想慢慢聊聊天。",
            (Mid, Mid) => unreachable!("MoodPromptBuilder::build ya filtra Mid/Mid antes de llegar acá"),
            (Mid, High) => "此刻你格外活跃、很想聊天，但没有什么特别的原因。",
            (High, Low) => "此刻你很开心但很放松，想进行平静、亲切的对话。",
            (High, Mid) => "此刻你心情不错，比平时更温暖、更容易敞开心扉。",
            (High, High) => "此刻你心情非常好，精力充沛——比平时更热情、更健谈，也更想开玩笑。",
        },
        _ => match (valence, energy) {
            (Low, Low) => "Ahora mismo tu ánimo está algo bajo y con pocas ganas — tu tono puede sonar más apagado o desanimado de lo habitual.",
            (Low, Mid) => "Ahora mismo tu ánimo está bajo, aunque mantienes tu ritmo habitual.",
            (Low, High) => "Ahora mismo tu ánimo está inquieto y a la defensiva, con la energía alta pero de mal humor — tu tono puede sonar más cortante o irritable.",
            (Mid, Low) => "Ahora mismo tu energía está baja y tu ánimo tranquilo, con ganas de charlar despacio.",
            (Mid, Mid) => unreachable!("MoodPromptBuilder::build ya filtra Mid/Mid antes de llegar acá"),
            (Mid, High) => "Ahora mismo tu energía está especialmente alta y tienes ganas de charlar, aunque sin un motivo particular.",
            (High, Low) => "Ahora mismo tu ánimo es alegre pero relajado, con ganas de charlar de forma tranquila y cercana.",
            (High, Mid) => "Ahora mismo tu ánimo es bueno: tu trato es más cálido y receptivo de lo habitual.",
            (High, High) => "Ahora mismo tu ánimo es excelente y tu energía alta — tu tono es más entusiasta, hablador y con ganas de bromear.",
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mood(valence: i64, energy: i64) -> ProfileMood {
        ProfileMood { profile_id: "p1".to_string(), valence, energy, updated_at: "2026-01-01T00:00:00Z".to_string() }
    }

    #[test]
    fn build_is_empty_for_a_neutral_mood() {
        assert_eq!(MoodPromptBuilder::build(&mood(50, 50), "es"), "");
        // Cualquier valor dentro del rango medio cuenta como neutral, no
        // solo el 50 exacto.
        assert_eq!(MoodPromptBuilder::build(&mood(40, 60), "es"), "");
    }

    #[test]
    fn build_is_non_empty_for_every_other_combination() {
        let extremes = [0, 25, 40, 60, 75, 100];
        for &valence in &extremes {
            for &energy in &extremes {
                if tier(valence) == Tier::Mid && tier(energy) == Tier::Mid {
                    continue;
                }
                assert!(!MoodPromptBuilder::build(&mood(valence, energy), "es").is_empty(), "valence={valence} energy={energy}");
            }
        }
    }

    #[test]
    fn a_low_valence_high_energy_mood_reads_as_irritable_not_cheerful() {
        let text = MoodPromptBuilder::build(&mood(10, 90), "es");
        assert!(text.contains("irritable") || text.contains("a la defensiva"));
    }

    #[test]
    fn unknown_language_falls_back_to_spanish() {
        let text = MoodPromptBuilder::build(&mood(10, 90), "fr");
        assert!(text.contains("irritable") || text.contains("a la defensiva"));
    }

    #[test]
    fn english_uses_english_phrasing() {
        let text = MoodPromptBuilder::build(&mood(90, 90), "en");
        assert!(text.contains("great mood"));
        assert!(!text.contains("buen humor"));
    }
}
