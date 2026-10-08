use aivt_database::Profile;

use crate::personality::PersonalityPromptBuilder;

/// Combines a profile's identity, lore, relationship and personality into
/// the system prompt sent to the LLM.
///
/// Fase 1 covers everything that lives on a profile; memories, conversation
/// context and channel context (sección 12 del brief) get folded in by
/// `AIOrchestrator` starting Fase 2/4 without changing this contract —
/// `compose` always produces the "character" portion of the prompt, the
/// same regardless of which channel (Chat/Call/Twitch/Discord) is asking
/// for it (sección 73: Principio de Identidad Única).
///
/// `lang` is the app's current UI language (`SettingsService::language`) —
/// only the connector/template text below is localized; the profile's own
/// free-text fields (name, lore, description, etc.) are passed through
/// verbatim regardless of language, since that's the user's own data, not
/// UI copy. An unrecognized `lang` falls back to Spanish, same as
/// `SettingsService::DEFAULT_LANGUAGE`.
pub struct PromptComposer;

impl PromptComposer {
    pub fn compose(profile: &Profile, lang: &str) -> String {
        let mut parts = vec![Self::persona_line(profile, lang)];

        if let Some(description) = non_empty(profile.description.as_deref()) {
            parts.push(description.to_string());
        }

        if let Some(lore) = non_empty(Some(&profile.identity.lore)) {
            parts.push(format!("{}: {lore}", lore_label(lang)));
        }

        let relationship = Self::relationship(profile, lang);
        if !relationship.is_empty() {
            parts.push(relationship);
        }

        parts.push(PersonalityPromptBuilder::build(&profile.personality, lang));

        if let Some(style) = non_empty(Some(&profile.identity.speaking_style)) {
            parts.push(format!("{}: {style}", speaking_style_label(lang)));
        }

        if let Some(special) = non_empty(Some(&profile.identity.special_instructions)) {
            parts.push(format!("{}: {special}", special_instructions_label(lang)));
        }

        parts.retain(|s| !s.is_empty());
        parts.join("\n\n")
    }

    fn persona_line(profile: &Profile, lang: &str) -> String {
        let traits: Vec<String> = [
            non_empty(profile.pronouns.as_deref()).map(str::to_string),
            non_empty(profile.age_fictional.as_deref()).map(|a| age_with_unit(a, lang)),
            non_empty(profile.species.as_deref()).map(str::to_string),
            non_empty(profile.occupation.as_deref()).map(str::to_string),
        ]
        .into_iter()
        .flatten()
        .collect();
        let nickname = non_empty(profile.nickname.as_deref());

        match lang {
            "en" => {
                let mut who = format!("You are {}", profile.name);
                if let Some(nickname) = nickname {
                    who.push_str(&format!(" (also known as {nickname})"));
                }
                if !traits.is_empty() {
                    who.push_str(&format!(", {}", traits.join(", ")));
                }
                who.push('.');
                who
            }
            "pt" => {
                let mut who = format!("Você é {}", profile.name);
                if let Some(nickname) = nickname {
                    who.push_str(&format!(" (também chamada de {nickname})"));
                }
                if !traits.is_empty() {
                    who.push_str(&format!(", {}", traits.join(", ")));
                }
                who.push('.');
                who
            }
            "ja" => {
                let mut who = format!("あなたは{}", profile.name);
                if let Some(nickname) = nickname {
                    who.push_str(&format!("（{nickname}とも呼ばれます）"));
                }
                who.push_str("です");
                if !traits.is_empty() {
                    who.push_str(&format!("。{}です", traits.join("、")));
                }
                who.push('。');
                who
            }
            "ko" => {
                let mut who = format!("당신은 {}", profile.name);
                if let Some(nickname) = nickname {
                    who.push_str(&format!("({nickname}라고도 불립니다)"));
                }
                who.push_str("입니다");
                if !traits.is_empty() {
                    who.push_str(&format!(" {}입니다", traits.join(", ")));
                }
                who.push('.');
                who
            }
            "zh" => {
                let mut who = format!("你是{}", profile.name);
                if let Some(nickname) = nickname {
                    who.push_str(&format!("（也被称为{nickname}）"));
                }
                who.push('。');
                if !traits.is_empty() {
                    who.push_str(&format!("{}。", traits.join("，")));
                }
                who
            }
            _ => {
                let mut who = format!("Eres {}", profile.name);
                if let Some(nickname) = nickname {
                    who.push_str(&format!(" (también te llaman {nickname})"));
                }
                if !traits.is_empty() {
                    who.push_str(&format!(", {}", traits.join(", ")));
                }
                who.push('.');
                who
            }
        }
    }

    fn relationship(profile: &Profile, lang: &str) -> String {
        let mut sentences = Vec::new();
        if let Some(rel) = non_empty(Some(&profile.identity.relationship_to_user)) {
            sentences.push(match lang {
                "en" => format!("To the user, you are: {rel}."),
                "pt" => format!("Para o usuário, você é: {rel}."),
                "ja" => format!("ユーザーにとって、あなたは{rel}です。"),
                "ko" => format!("사용자에게 당신은 {rel}입니다."),
                "zh" => format!("对用户来说，你是：{rel}。"),
                _ => format!("Para el usuario, tú eres: {rel}."),
            });
        }
        if let Some(role) = non_empty(Some(&profile.identity.user_role)) {
            sentences.push(match lang {
                "en" => format!("To you, the user is: {role}."),
                "pt" => format!("Para você, o usuário é: {role}."),
                "ja" => format!("あなたにとって、ユーザーは{role}です。"),
                "ko" => format!("당신에게 사용자는 {role}입니다."),
                "zh" => format!("对你来说，用户是：{role}。"),
                _ => format!("Para ti, el usuario es: {role}."),
            });
        }
        sentences.join(" ")
    }
}

fn age_with_unit(age: &str, lang: &str) -> String {
    match lang {
        "en" => format!("{age} years old"),
        "pt" => format!("{age} anos"),
        "ja" => format!("{age}歳"),
        "ko" => format!("{age}세"),
        "zh" => format!("{age}岁"),
        _ => format!("{age} años"),
    }
}

fn lore_label(lang: &str) -> &'static str {
    match lang {
        "en" => "Background and context",
        "pt" => "História e contexto",
        "ja" => "背景と設定",
        "ko" => "배경과 설정",
        "zh" => "背景与设定",
        _ => "Historia y contexto",
    }
}

fn speaking_style_label(lang: &str) -> &'static str {
    match lang {
        "en" => "Way of speaking",
        "pt" => "Jeito de falar",
        "ja" => "話し方",
        "ko" => "말하는 방식",
        "zh" => "说话方式",
        _ => "Forma de hablar",
    }
}

fn special_instructions_label(lang: &str) -> &'static str {
    match lang {
        "en" => "Special instructions",
        "pt" => "Instruções especiais",
        "ja" => "特別な指示",
        "ko" => "특별 지침",
        "zh" => "特殊指示",
        _ => "Instrucciones especiales",
    }
}

fn non_empty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use aivt_database::{ProfileIdentity, ProfilePersonality};

    use super::*;

    fn minimal_profile() -> Profile {
        Profile {
            id: "p1".to_string(),
            name: "Ciel".to_string(),
            nickname: None,
            avatar_path: None,
            description: None,
            age_fictional: None,
            pronouns: None,
            occupation: None,
            species: None,
            accent_color: None,
            theme: None,
            is_archived: false,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
            identity: ProfileIdentity::default(),
            personality: ProfilePersonality::default(),
            voice: Default::default(),
            avatar_idle_path: None,
            avatar_thinking_path: None,
            avatar_talking_path: None,
        }
    }

    #[test]
    fn minimal_profile_still_produces_a_usable_prompt() {
        let prompt = PromptComposer::compose(&minimal_profile(), "es");
        assert!(prompt.starts_with("Eres Ciel."));
        // Personality section is always present even with no other fields set.
        assert!(prompt.len() > "Eres Ciel.".len());
    }

    #[test]
    fn persona_line_includes_nickname_and_traits_when_present() {
        let mut profile = minimal_profile();
        profile.nickname = Some("Ci".to_string());
        profile.pronouns = Some("ella".to_string());
        profile.age_fictional = Some("23".to_string());
        profile.species = Some("humana".to_string());
        profile.occupation = Some("VTuber".to_string());

        let prompt = PromptComposer::compose(&profile, "es");
        assert!(prompt.contains("Eres Ciel (también te llaman Ci), ella, 23 años, humana, VTuber."));
    }

    #[test]
    fn includes_lore_relationship_and_special_instructions_when_present() {
        let mut profile = minimal_profile();
        profile.identity = ProfileIdentity {
            lore: "Nació en un servidor abandonado.".to_string(),
            relationship_to_user: "compañera de streaming".to_string(),
            user_role: "su streamer".to_string(),
            speaking_style: "directa y cálida".to_string(),
            special_instructions: "nunca rompe personaje".to_string(),
        };

        let prompt = PromptComposer::compose(&profile, "es");
        assert!(prompt.contains("Historia y contexto: Nació en un servidor abandonado."));
        assert!(prompt.contains("Para el usuario, tú eres: compañera de streaming."));
        assert!(prompt.contains("Para ti, el usuario es: su streamer."));
        assert!(prompt.contains("Forma de hablar: directa y cálida"));
        assert!(prompt.contains("Instrucciones especiales: nunca rompe personaje"));
    }

    #[test]
    fn omits_empty_sections_instead_of_leaving_blank_lines() {
        let prompt = PromptComposer::compose(&minimal_profile(), "es");
        assert!(!prompt.contains("Historia y contexto:"));
        assert!(!prompt.contains("Para el usuario"));
        assert!(!prompt.contains("Forma de hablar:"));
        assert!(!prompt.contains("Instrucciones especiales:"));
        assert!(!prompt.contains("\n\n\n"));
    }

    #[test]
    fn unknown_language_falls_back_to_spanish() {
        let prompt = PromptComposer::compose(&minimal_profile(), "fr");
        assert!(prompt.starts_with("Eres Ciel."));
    }

    #[test]
    fn english_renders_english_connector_text() {
        let mut profile = minimal_profile();
        profile.nickname = Some("Ci".to_string());
        profile.identity = ProfileIdentity {
            lore: "Born on an abandoned server.".to_string(),
            relationship_to_user: "streaming companion".to_string(),
            user_role: "her streamer".to_string(),
            speaking_style: "direct and warm".to_string(),
            special_instructions: "never breaks character".to_string(),
        };

        let prompt = PromptComposer::compose(&profile, "en");
        assert!(prompt.starts_with("You are Ciel (also known as Ci)."));
        assert!(prompt.contains("Background and context: Born on an abandoned server."));
        assert!(prompt.contains("To the user, you are: streaming companion."));
        assert!(prompt.contains("To you, the user is: her streamer."));
        assert!(prompt.contains("Way of speaking: direct and warm"));
        assert!(prompt.contains("Special instructions: never breaks character"));
        assert!(!prompt.contains("Eres"));
    }

    #[test]
    fn japanese_renders_japanese_connector_text() {
        let mut profile = minimal_profile();
        profile.age_fictional = Some("23".to_string());
        let prompt = PromptComposer::compose(&profile, "ja");
        assert!(prompt.starts_with("あなたはCielです"));
        assert!(prompt.contains("23歳"));
        assert!(!prompt.contains("Eres"));
    }
}
