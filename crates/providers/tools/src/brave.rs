use async_trait::async_trait;
use serde::Deserialize;

use crate::error::ToolError;
use crate::tool::Tool;

/// How many results to show the model — enough to actually answer most
/// questions (weather, current events, a fact needing a couple sources to
/// corroborate) without spending an unreasonable number of tokens on a
/// single tool result.
const MAX_RESULTS: usize = 5;

/// A single web-search tool backed by the Brave Search API — see
/// docs/DEVELOPMENT_STATUS.md's Fase 10b section for why Brave was chosen
/// over SearXNG/DuckDuckGo.
///
/// `lang` (`SettingsService::language`) localizes everything this tool
/// sends the model — description, parameter schema, error/fallback text —
/// same convention as `PromptComposer` (falls back to Spanish for an
/// unrecognized value). Resolved once at construction (tools are built
/// fresh per request), same reasoning as `CurrentTimeTool`.
pub struct BraveSearchTool {
    client: reqwest::Client,
    api_key: String,
    lang: String,
}

impl BraveSearchTool {
    pub fn new(api_key: impl Into<String>, lang: impl Into<String>) -> Self {
        Self { client: reqwest::Client::new(), api_key: api_key.into(), lang: lang.into() }
    }
}

#[async_trait]
impl Tool for BraveSearchTool {
    fn name(&self) -> &str {
        "web_search"
    }

    fn description(&self) -> &str {
        match self.lang.as_str() {
            "en" => {
                "Searches the web for current information — weather, the time in another \
                 country, news, or any fact you don't know for certain. Returns a list of \
                 results with title, summary and URL. For weather or anything else that \
                 depends on a location: include a real city in the search, never search for \
                 something like 'my location' literally — use the user's timezone (system \
                 message) as a hint if you weren't given a city, or ask them directly."
            }
            "pt" => {
                "Busca informação atual na web — clima, hora em outro país, notícias, ou \
                 qualquer fato que você não saiba com certeza. Retorna uma lista de \
                 resultados com título, resumo e URL. Para clima ou qualquer outra coisa que \
                 dependa de uma localização: inclua uma cidade real na busca, nunca busque \
                 algo como 'minha localização' literalmente — use o fuso horário do usuário \
                 (mensagem do sistema) como pista se não te deram uma cidade, ou pergunte \
                 diretamente."
            }
            "ja" => {
                "ウェブで最新の情報を検索する — 天気、他の国の時刻、ニュース、または確実に知らない事実など。\
                 タイトル・要約・URLを含む結果一覧を返す。天気など場所に依存する検索では、必ず実在する都市名を \
                 含めること。「自分の場所」のような文字通りの検索は絶対にしない — 都市を指定されていない場合は \
                 ユーザーのタイムゾーン（システムメッセージ）を手がかりにするか、直接尋ねること。"
            }
            "ko" => {
                "웹에서 최신 정보를 검색한다 — 날씨, 다른 나라의 시간, 뉴스, 또는 확실히 모르는 사실 등. \
                 제목, 요약, URL이 포함된 결과 목록을 반환한다. 날씨처럼 위치에 따라 달라지는 검색에는 \
                 반드시 실제 도시명을 포함할 것 — '내 위치'처럼 문자 그대로 검색하지 말 것. 도시를 \
                 알려주지 않았다면 사용자의 시간대(시스템 메시지)를 단서로 삼거나 직접 물어볼 것."
            }
            "zh" => {
                "在网上搜索最新信息——天气、其他国家的时间、新闻，或任何你不确定的事实。返回包含标题、摘要和 \
                 URL 的结果列表。对于天气等依赖位置的查询：搜索时要带上一个真实的城市名，绝不要照字面意思搜索 \
                 “我的位置”之类的内容——如果没有给出城市，就用用户的时区（系统消息）作为线索，或者直接问用户。"
            }
            _ => {
                "Busca información actual en la web — clima, hora en otro país, noticias, \
                 o cualquier hecho que no sepas con certeza. Devuelve una lista de \
                 resultados con título, resumen y URL. Para clima o cualquier otra cosa que \
                 dependa de una ubicación: incluí una ciudad real en la búsqueda, nunca \
                 busques algo como 'mi ubicación' literalmente — usá la zona horaria del \
                 usuario (mensaje del sistema) como pista si no te dieron una ciudad, o \
                 preguntale directamente."
            }
        }
    }

    fn parameters_schema(&self) -> serde_json::Value {
        let description = match self.lang.as_str() {
            "en" => "The search terms — in whichever language will best find the result.",
            "pt" => "Os termos de busca — no idioma que melhor for encontrar o resultado.",
            "ja" => "検索キーワード — 結果が見つかりやすい言語で。",
            "ko" => "검색어 — 결과를 가장 잘 찾을 수 있는 언어로.",
            "zh" => "搜索关键词——使用最能找到结果的语言。",
            _ => "Los términos de búsqueda — en el idioma que mejor vaya a encontrar el resultado.",
        };
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": description,
                }
            },
            "required": ["query"],
        })
    }

    async fn execute(&self, arguments_json: &str) -> Result<String, ToolError> {
        let query = parse_query_argument(arguments_json, &self.lang)?;

        let response = self
            .client
            .get("https://api.search.brave.com/res/v1/web/search")
            .query(&[("q", query.as_str())])
            .header("Accept", "application/json")
            .header("X-Subscription-Token", &self.api_key)
            .send()
            .await
            .map_err(|err| ToolError::Network(err.to_string()))?;

        let status = response.status();
        let body = response.text().await.map_err(|err| ToolError::Network(err.to_string()))?;
        if !status.is_success() {
            return Err(ToolError::BackendError(format!("HTTP {status}: {body}")));
        }

        let parsed: BraveWebSearchResponse =
            serde_json::from_str(&body).map_err(|err| ToolError::BackendError(err.to_string()))?;
        Ok(format_results(&parsed, &self.lang))
    }
}

/// Pulls `query` out of the model-supplied arguments — kept separate from
/// `execute` so it's testable without any HTTP involved.
fn parse_query_argument(arguments_json: &str, lang: &str) -> Result<String, ToolError> {
    let value: serde_json::Value =
        serde_json::from_str(arguments_json).map_err(|err| ToolError::InvalidArguments(err.to_string()))?;
    value
        .get("query")
        .and_then(|q| q.as_str())
        .filter(|q| !q.trim().is_empty())
        .map(str::to_string)
        .ok_or_else(|| ToolError::InvalidArguments(missing_query_message(lang)))
}

fn missing_query_message(lang: &str) -> String {
    match lang {
        "en" => "missing 'query' field".to_string(),
        "pt" => "falta o campo 'query'".to_string(),
        "ja" => "'query' フィールドがありません".to_string(),
        "ko" => "'query' 필드가 없습니다".to_string(),
        "zh" => "缺少 'query' 字段".to_string(),
        _ => "falta el campo 'query'".to_string(),
    }
}

#[derive(Debug, Deserialize)]
struct BraveWebSearchResponse {
    web: Option<BraveWebResults>,
}

#[derive(Debug, Deserialize)]
struct BraveWebResults {
    #[serde(default)]
    results: Vec<BraveResult>,
}

#[derive(Debug, Deserialize)]
struct BraveResult {
    title: String,
    #[serde(default)]
    description: String,
    url: String,
}

/// Formats Brave's response into the plain text sent back to the model as
/// the tool's result — a pure function (no HTTP) so this is unit-testable
/// against a hand-written sample response, without needing an HTTP-mocking
/// dependency the workspace doesn't otherwise use. `lang` only affects the
/// no-results fallback line — a real result's `title`/`description` are
/// Brave's own data, not ours to translate.
fn format_results(response: &BraveWebSearchResponse, lang: &str) -> String {
    let results = response.web.as_ref().map(|w| w.results.as_slice()).unwrap_or_default();
    if results.is_empty() {
        return no_results_message(lang).to_string();
    }

    results
        .iter()
        .take(MAX_RESULTS)
        .map(|r| format!("- {}: {} ({})", r.title, r.description, r.url))
        .collect::<Vec<_>>()
        .join("\n")
}

fn no_results_message(lang: &str) -> &'static str {
    match lang {
        "en" => "No results found.",
        "pt" => "Nenhum resultado encontrado.",
        "ja" => "結果が見つかりませんでした。",
        "ko" => "결과를 찾을 수 없습니다.",
        "zh" => "未找到结果。",
        _ => "No se encontraron resultados.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_query_argument() {
        assert_eq!(parse_query_argument(r#"{"query":"clima en Bogotá"}"#, "es").unwrap(), "clima en Bogotá");
    }

    #[test]
    fn rejects_missing_query() {
        assert!(matches!(parse_query_argument("{}", "es"), Err(ToolError::InvalidArguments(_))));
    }

    #[test]
    fn rejects_blank_query() {
        assert!(matches!(parse_query_argument(r#"{"query":"   "}"#, "es"), Err(ToolError::InvalidArguments(_))));
    }

    #[test]
    fn rejects_malformed_json() {
        assert!(matches!(parse_query_argument("not json", "es"), Err(ToolError::InvalidArguments(_))));
    }

    #[test]
    fn formats_results_as_a_bulleted_list() {
        let response = BraveWebSearchResponse {
            web: Some(BraveWebResults {
                results: vec![
                    BraveResult { title: "Clima en Bogotá".to_string(), description: "18°C, nublado".to_string(), url: "https://example.com/1".to_string() },
                    BraveResult { title: "Pronóstico extendido".to_string(), description: "Lluvias esta tarde".to_string(), url: "https://example.com/2".to_string() },
                ],
            }),
        };

        let text = format_results(&response, "es");
        assert!(text.contains("Clima en Bogotá: 18°C, nublado (https://example.com/1)"));
        assert!(text.contains("Pronóstico extendido: Lluvias esta tarde (https://example.com/2)"));
    }

    #[test]
    fn caps_results_at_max_results() {
        let results = (0..10)
            .map(|i| BraveResult { title: format!("r{i}"), description: String::new(), url: format!("https://example.com/{i}") })
            .collect();
        let response = BraveWebSearchResponse { web: Some(BraveWebResults { results }) };

        let text = format_results(&response, "es");
        assert_eq!(text.lines().count(), MAX_RESULTS);
    }

    #[test]
    fn no_results_produces_a_clear_message() {
        let response = BraveWebSearchResponse { web: Some(BraveWebResults { results: vec![] }) };
        assert_eq!(format_results(&response, "es"), "No se encontraron resultados.");
    }

    #[test]
    fn missing_web_field_is_treated_as_no_results() {
        let response = BraveWebSearchResponse { web: None };
        assert_eq!(format_results(&response, "es"), "No se encontraron resultados.");
    }

    #[test]
    fn no_results_message_follows_the_language() {
        let response = BraveWebSearchResponse { web: None };
        assert_eq!(format_results(&response, "en"), "No results found.");
    }

    #[test]
    fn description_and_schema_follow_the_configured_language() {
        let tool = BraveSearchTool::new("key", "en");
        assert!(tool.description().contains("Searches the web"));
        assert!(!tool.description().contains("Busca información"));
        let schema = tool.parameters_schema();
        assert!(schema["properties"]["query"]["description"].as_str().unwrap().contains("search terms"));
    }

    #[test]
    fn unknown_language_falls_back_to_spanish() {
        let tool = BraveSearchTool::new("key", "fr");
        assert!(tool.description().contains("Busca información"));
    }
}
