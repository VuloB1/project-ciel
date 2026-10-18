//! Conversation Lab: a fixed set of everyday situations run through the real
//! `AIOrchestrator` against a real model, with cheap local checks on the replies
//! (length, unrequested lists, ending in a question, assistant-isms).
//!
//! Its job is to turn "this prompt feels more natural" into numbers you can compare
//! before/after a prompt change. It is test-only and ignored by default because it
//! needs a model server:
//!
//! ```text
//! LAB_MODEL=qwen3-vl-8b cargo test -p aivt-orchestrator lab -- --ignored --nocapture
//! ```
//!
//! Optional: `LAB_BASE_URL` (default `http://localhost:1234/v1`), `LAB_LANG` (default `es`,
//! the scenarios themselves are Spanish), `LAB_TEMPERATURE` (default 0.7).
//! Scenario checks are heuristics, not a quality score: read the printed replies too.

use aivt_database::MessageRole as DbMessageRole;

use crate::Channel;

/// Phrases that mark the model falling back into "helpful assistant" mode.
const ASSISTANT_ISMS: &[&str] = &[
    "claro, aquí tienes",
    "aquí tienes",
    "espero que te sirva",
    "¿hay algo más",
    "si quieres, puedo",
    "si quieres puedo",
    "por supuesto",
    "¿en qué puedo ayudar",
    "como asistente",
    "como ia",
    "como modelo de lenguaje",
    "no dudes en",
    "estoy aquí para ayudarte",
    "here you go",
    "let me know if",
    "as an ai",
];

#[derive(Debug, PartialEq)]
pub struct Metrics {
    pub words: usize,
    pub ends_with_question: bool,
    pub has_list: bool,
    pub has_heading: bool,
    pub assistant_isms: Vec<&'static str>,
}

impl Metrics {
    pub fn of(reply: &str) -> Self {
        let lower = reply.to_lowercase();
        Self {
            words: reply.split_whitespace().count(),
            ends_with_question: matches!(reply.trim_end().chars().last(), Some('?' | '？')),
            has_list: reply.lines().any(is_list_line),
            has_heading: reply.lines().any(|l| l.trim_start().starts_with('#')),
            assistant_isms: ASSISTANT_ISMS.iter().copied().filter(|p| lower.contains(p)).collect(),
        }
    }
}

fn is_list_line(line: &str) -> bool {
    let line = line.trim_start();
    let numbered = line.split_once(". ").is_some_and(|(n, _)| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()));
    line.starts_with("- ") || line.starts_with("* ") || line.starts_with("• ") || numbered
}

/// What a good reply looks like for one scenario. Assistant-isms and headings are never wanted.
pub struct Expect {
    pub max_words: Option<usize>,
    pub min_words: usize,
    pub allow_list: bool,
    pub allow_question: bool,
}

pub struct Scenario {
    pub name: &'static str,
    /// Earlier turns as (is_user, text), oldest first, before the user's message under test.
    pub before: &'static [(bool, &'static str)],
    pub user: &'static str,
    pub channel: Channel,
    pub expect: Expect,
}

impl Scenario {
    pub fn problems(&self, m: &Metrics) -> Vec<String> {
        let e = &self.expect;
        let mut out = Vec::new();
        if let Some(max) = e.max_words {
            if m.words > max {
                out.push(format!("{} words (max {max})", m.words));
            }
        }
        if m.words < e.min_words {
            out.push(format!("{} words (min {})", m.words, e.min_words));
        }
        if m.has_list && !e.allow_list {
            out.push("unrequested list".to_string());
        }
        if m.has_heading {
            out.push("heading".to_string());
        }
        if m.ends_with_question && !e.allow_question {
            out.push("ends with a question".to_string());
        }
        for phrase in &m.assistant_isms {
            out.push(format!("assistant-ism: \"{phrase}\""));
        }
        out
    }
}

const fn short(max_words: usize) -> Expect {
    Expect { max_words: Some(max_words), min_words: 1, allow_list: false, allow_question: true }
}

const fn statement(max_words: usize) -> Expect {
    Expect { max_words: Some(max_words), min_words: 1, allow_list: false, allow_question: false }
}

const HELP: Expect = Expect { max_words: None, min_words: 25, allow_list: true, allow_question: true };

const fn s(name: &'static str, user: &'static str, expect: Expect) -> Scenario {
    Scenario { name, before: &[], user, channel: Channel::Chat, expect }
}

const fn call(name: &'static str, user: &'static str, max_words: usize) -> Scenario {
    Scenario { name, before: &[], user, channel: Channel::Call, expect: short(max_words) }
}

pub const SCENARIOS: &[Scenario] = &[
    // Tiny casual messages: one short reaction.
    s("saludo", "holaa", short(25)),
    s("risa", "jajaja sii", short(25)),
    s("aburrido", "toy aburrido", short(45)),
    s("sueño", "tengo sueño", short(45)),
    s("incredulidad", "JAJAJA no puede ser", short(35)),
    s("ok", "ok", short(25)),
    s("buenos días", "buenos días", short(35)),
    s("nah", "nah", short(25)),
    // Feelings: listen, don't fix.
    s("día horrible", "hoy fue un día horrible", short(80)),
    s("sin consejos", "no quiero consejos, solo quería contártelo", statement(60)),
    s("solo", "me siento solo hoy", short(80)),
    s("discusión", "tuve una discusión con mi mejor amigo", short(90)),
    s("nervios", "mañana tengo examen y estoy nervioso", short(90)),
    // Everyday conversation.
    s("gatos", "¿qué opinas de los gatos?", short(90)),
    s("teclado", "me compré un teclado nuevo", short(80)),
    s("película", "acabo de ver una película muy buena", short(80)),
    s("ejercicio", "creo que voy a empezar a hacer ejercicio", short(90)),
    s("gustos", "¿qué te gusta hacer?", short(90)),
    s("cómo estás", "¿cómo estás?", short(55)),
    s("lunes", "los lunes son lo mejor", short(80)),
    s("chiste", "cuéntame un chiste", short(70)),
    s("eres ia", "¿eres una IA?", short(70)),
    // Continuity: the reply must build on the earlier turns.
    Scenario {
        name: "ayer",
        before: &[(true, "ayer me fue mal en la entrevista"), (false, "uf, qué pena. ¿Cómo te quedó el ánimo?")],
        user: "me acordé de lo que hablamos ayer",
        channel: Channel::Chat,
        expect: short(90),
    },
    Scenario {
        name: "cambio de tema",
        before: &[(true, "estaba pensando en mudarme"), (false, "¿a otra ciudad o a otro país?")],
        user: "oye, cambiando de tema, ¿viste algo bueno últimamente?",
        channel: Channel::Chat,
        expect: short(90),
    },
    // Real requests for help: detail and structure are fine.
    s("embeddings", "explícame cómo funcionan los embeddings", HELP),
    s("correo", "ayúdame a escribir un correo para pedir vacaciones", HELP),
    s("memoria", "explica cómo funciona la memoria de un ordenador", HELP),
    // Voice call: spoken style.
    call("llamada: saludo", "hola, ¿me escuchas bien?", 35),
    call("llamada: día", "cuéntame qué tal tu día", 55),
    call("llamada: opinión", "¿tú qué harías en mi lugar si te ofrecen otro trabajo?", 60),
];

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::sync::Arc;

    use futures::StreamExt;

    use aivt_database::Message;
    use aivt_provider_llm::OpenAiCompatibleProvider;

    use super::*;
    use crate::orchestrator::tests::{sample_message, sample_profile};
    use crate::AIOrchestrator;

    #[test]
    fn metrics_detect_lists_headings_questions_and_assistant_isms() {
        let m = Metrics::of("# Título\n- uno\n- dos\nClaro, aquí tienes. ¿Algo más?");
        assert!(m.has_heading && m.has_list && m.ends_with_question);
        assert_eq!(m.assistant_isms, vec!["claro, aquí tienes", "aquí tienes"]);

        let plain = Metrics::of("jajaja sí, me pasa igual");
        assert_eq!(plain, Metrics { words: 5, ends_with_question: false, has_list: false, has_heading: false, assistant_isms: vec![] });
        assert!(Metrics::of("1. primero\n2. segundo").has_list);
        assert!(!Metrics::of("en 2024. Fue bueno").has_list);
    }

    #[test]
    fn problems_follow_the_scenarios_expectations() {
        let casual = &SCENARIOS[1];
        assert!(casual.problems(&Metrics::of("jajaja yo igual")).is_empty());
        let long_reply = "palabra ".repeat(60);
        assert!(casual.problems(&Metrics::of(&long_reply)).iter().any(|p| p.contains("max")));
        let help = SCENARIOS.iter().find(|s| s.name == "embeddings").unwrap();
        assert!(help.problems(&Metrics::of(&format!("- a\n- b\n{}", "palabra ".repeat(30)))).is_empty());
    }

    #[test]
    fn the_lab_has_unique_scenario_names() {
        let names: HashSet<_> = SCENARIOS.iter().map(|s| s.name).collect();
        assert_eq!(names.len(), SCENARIOS.len());
    }

    /// Needs a running OpenAI-compatible server (LM Studio, Ollama...). See the module docs.
    #[tokio::test]
    #[ignore = "needs a model server, run with LAB_MODEL=... -- --ignored --nocapture"]
    async fn run_against_a_real_model() {
        let model = std::env::var("LAB_MODEL").expect("set LAB_MODEL to the model name");
        let base_url = std::env::var("LAB_BASE_URL").unwrap_or_else(|_| "http://localhost:1234/v1".to_string());
        let lang = std::env::var("LAB_LANG").unwrap_or_else(|_| "es".to_string());
        let temperature: f32 = std::env::var("LAB_TEMPERATURE").ok().and_then(|t| t.parse().ok()).unwrap_or(0.7);
        let provider = Arc::new(OpenAiCompatibleProvider::new(base_url, None));

        let (mut clean, mut seen, mut repeated, mut questions, mut casual) = (0, HashSet::new(), 0, 0, 0);
        for scenario in SCENARIOS {
            let mut history: Vec<Message> = scenario
                .before
                .iter()
                .map(|(is_user, text)| sample_message(if *is_user { DbMessageRole::User } else { DbMessageRole::Assistant }, text))
                .collect();
            history.push(sample_message(DbMessageRole::User, scenario.user));

            let orchestrator = AIOrchestrator::new(provider.clone(), model.clone(), Some(temperature), None).with_channel(scenario.channel);
            let mut reply = String::new();
            let mut stream = orchestrator.stream_respond(&sample_profile(), &history, None, &[], None, None, &lang);
            while let Some(chunk) = stream.next().await {
                reply.push_str(&chunk.expect("the model server returned an error").delta);
            }
            let reply = strip_thinking(&reply);

            let metrics = Metrics::of(&reply);
            if scenario.expect.max_words.is_some() {
                casual += 1;
                questions += usize::from(metrics.ends_with_question);
            }
            let problems = scenario.problems(&metrics);
            if !seen.insert(reply.to_lowercase()) {
                repeated += 1;
            }
            if problems.is_empty() {
                clean += 1;
            }
            let verdict = if problems.is_empty() { "ok".to_string() } else { problems.join("; ") };
            println!("[{:<18}] {} -> {}\n    {}\n", scenario.name, scenario.user, verdict, reply.replace('\n', "\n    "));
        }
        println!(
            "LAB: {clean}/{} scenarios clean, {repeated} repeated replies, {questions}/{casual} casual replies end with a question (model {model})",
            SCENARIOS.len()
        );
    }

    /// Does the model actually ask for the web search tool when told to search? Uses the REAL
    /// tool description (so the model sees what it sees in the app) with a canned result:
    /// `LAB_MODEL=... cargo test -p aivt-orchestrator searches_when_asked -- --ignored --nocapture`.
    #[tokio::test]
    #[ignore = "needs a model server, run with LAB_MODEL=... -- --ignored --nocapture"]
    async fn searches_when_asked() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        use aivt_provider_tools::{BraveSearchTool, Tool, ToolError};

        struct CountingSearch {
            real: BraveSearchTool,
            calls: Arc<AtomicUsize>,
        }
        #[async_trait::async_trait]
        impl Tool for CountingSearch {
            fn name(&self) -> &str {
                self.real.name()
            }
            fn description(&self) -> &str {
                self.real.description()
            }
            fn parameters_schema(&self) -> serde_json::Value {
                self.real.parameters_schema()
            }
            async fn execute(&self, arguments_json: &str) -> Result<String, ToolError> {
                println!("    [tool called with {arguments_json}]");
                self.calls.fetch_add(1, Ordering::SeqCst);
                Ok("1. Blender 5.0 released - new features include a faster renderer.
2. Blender Foundation announces a new open movie.".to_string())
            }
        }

        let model = std::env::var("LAB_MODEL").expect("set LAB_MODEL to the model name");
        let base_url = std::env::var("LAB_BASE_URL").unwrap_or_else(|_| "http://localhost:1234/v1".to_string());
        let provider = Arc::new(OpenAiCompatibleProvider::new(base_url, None));

        for (channel, text) in [
            (Channel::Chat, "busca en internet las últimas noticias sobre Blender"),
            (Channel::Call, "oye, busca en internet qué hay de nuevo en Blender"),
            (Channel::Call, "¿puedes buscar en la web cuándo sale el siguiente Blender?"),
        ] {
            let calls = Arc::new(AtomicUsize::new(0));
            let tool = CountingSearch { real: BraveSearchTool::new("dummy-key".to_string(), "es"), calls: calls.clone() };
            let orchestrator = AIOrchestrator::new(provider.clone(), model.clone(), Some(0.7), None).with_channel(channel).with_tools(vec![Arc::new(tool)]);
            let history = vec![sample_message(DbMessageRole::User, text)];
            let mut reply = String::new();
            let mut stream = orchestrator.stream_respond(&sample_profile(), &history, None, &[], None, None, "es");
            while let Some(chunk) = stream.next().await {
                reply.push_str(&chunk.expect("the model server returned an error").delta);
            }
            println!("[{channel:?}] {text}
    searched: {} | reply: {}
", calls.load(Ordering::SeqCst) > 0, strip_thinking(&reply));
        }
    }

    /// Reasoning models put their chain of thought in `<think>` tags: not part of the reply.
    fn strip_thinking(reply: &str) -> String {
        match (reply.find("<think>"), reply.find("</think>")) {
            (Some(start), Some(end)) if end > start => format!("{}{}", &reply[..start], &reply[end + "</think>".len()..]).trim().to_string(),
            _ => reply.trim().to_string(),
        }
    }
}
