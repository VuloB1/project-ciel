use async_trait::async_trait;
use chrono::{DateTime, Utc};
use chrono_tz::Tz;

use crate::error::ToolError;
use crate::tool::Tool;

/// Computes the current time directly (no network call) rather than
/// searching the web for it — a live-testing session found the model
/// asking `web_search` for "hora actual en Chile" and getting back a page
/// *about* a Chile time website, not an actual time, because general web
/// search has no structured answer for "what time is it right now
/// somewhere" the way it sometimes does for weather. Time zones are a
/// deterministic calculation, not something to look up.
///
/// `lang` (`SettingsService::language`) localizes everything this tool
/// sends the model — description, parameter schema, error text — same
/// convention as `PromptComposer` (falls back to Spanish for an
/// unrecognized value). It's resolved once at construction time (tools are
/// built fresh per request, see the command/service call sites) rather than
/// passed to every trait method, since `Tool::description`/
/// `parameters_schema` don't take arguments.
pub struct CurrentTimeTool {
    lang: String,
}

impl CurrentTimeTool {
    pub fn new(lang: impl Into<String>) -> Self {
        Self { lang: lang.into() }
    }
}

#[async_trait]
impl Tool for CurrentTimeTool {
    fn name(&self) -> &str {
        "get_current_time"
    }

    fn description(&self) -> &str {
        match self.lang.as_str() {
            "en" => {
                "Returns the current date and time in a specific timezone — use it for \
                 questions about what time or day it is in ANOTHER country or city, not the \
                 user's own. The user's own current date/time was already given to you \
                 directly, don't call this tool for that. Don't use web_search for another \
                 place's time either: a search won't give you the real time, only pages \
                 talking about it."
            }
            "pt" => {
                "Retorna a data e hora atuais em um fuso horário específico — use para \
                 perguntas sobre que horas são ou que dia é em OUTRO país ou cidade, \
                 diferente do usuário. A data/hora do próprio usuário já foi te dada \
                 diretamente, não chame esta ferramenta para isso. Também não use web_search \
                 para a hora de outro lugar: uma busca não vai te dar a hora real, só páginas \
                 que falam sobre o assunto."
            }
            "ja" => {
                "指定したタイムゾーンの現在の日付と時刻を返す。ユーザー本人とは別の国や都市の時刻・曜日を尋ねられたときに使う。\
                 ユーザー自身の現在の日付・時刻はすでに直接伝えてあるので、それにはこのツールを呼ばないこと。\
                 他の場所の時刻を調べるのに web_search も使わないこと — 検索では実際の時刻は得られず、その話題に関するページが出るだけ。"
            }
            "ko" => {
                "특정 시간대의 현재 날짜와 시간을 반환한다. 사용자 본인이 아닌 다른 나라나 도시의 시간/요일을 물어볼 때 사용한다. \
                 사용자 본인의 현재 날짜/시간은 이미 직접 전달되었으므로 그것을 위해 이 도구를 호출하지 말 것. \
                 다른 지역의 시간을 알아보는 데 web_search도 사용하지 말 것 — 검색으로는 실제 시간을 알 수 없고, 관련 페이지만 나온다."
            }
            "zh" => {
                "返回指定时区的当前日期和时间——用于询问用户所在地以外的其他国家或城市现在是几点/星期几。\
                 用户自己所在地的当前日期/时间已经直接提供给你了，不要为此调用这个工具。\
                 查询其他地方的时间也不要用 web_search——搜索并不能给你真实的时间，只能找到讨论这个话题的网页。"
            }
            _ => {
                "Devuelve la fecha y hora actual en una zona horaria específica — úsala para \
                 preguntas sobre qué hora o qué día es en OTRO país o ciudad distinto al del \
                 usuario. Para la fecha/hora del usuario mismo ya se te dio ese dato \
                 directamente, no llames esta herramienta para eso. No uses web_search para \
                 horas de otro lugar tampoco: una búsqueda no te va a dar la hora real, solo \
                 páginas que hablan del tema."
            }
        }
    }

    fn parameters_schema(&self) -> serde_json::Value {
        let description = match self.lang.as_str() {
            "en" => {
                "IANA timezone identifier ('Continent/City'), e.g. 'America/Santiago' for \
                 Chile, 'Europe/Madrid' for Spain, 'Asia/Tokyo' for Japan, \
                 'America/Mexico_City' for Mexico, 'America/Argentina/Buenos_Aires' for \
                 Argentina."
            }
            "pt" => {
                "Identificador de fuso horário no formato IANA ('Continente/Cidade'), por \
                 exemplo 'America/Santiago' para o Chile, 'Europe/Madrid' para a Espanha, \
                 'Asia/Tokyo' para o Japão, 'America/Mexico_City' para o México, \
                 'America/Argentina/Buenos_Aires' para a Argentina."
            }
            "ja" => {
                "IANAタイムゾーン識別子（「大陸/都市」形式）。例: チリなら'America/Santiago'、\
                 スペインなら'Europe/Madrid'、日本なら'Asia/Tokyo'、メキシコなら'America/Mexico_City'、\
                 アルゼンチンなら'America/Argentina/Buenos_Aires'。"
            }
            "ko" => {
                "IANA 시간대 식별자('대륙/도시' 형식). 예: 칠레는 'America/Santiago', 스페인은 \
                 'Europe/Madrid', 일본은 'Asia/Tokyo', 멕시코는 'America/Mexico_City', \
                 아르헨티나는 'America/Argentina/Buenos_Aires'."
            }
            "zh" => {
                "IANA 时区标识符（“大洲/城市”格式），例如智利用 'America/Santiago'，西班牙用 \
                 'Europe/Madrid'，日本用 'Asia/Tokyo'，墨西哥用 'America/Mexico_City'，\
                 阿根廷用 'America/Argentina/Buenos_Aires'。"
            }
            _ => {
                "Identificador de zona horaria en formato IANA ('Continente/Ciudad'), por \
                 ejemplo 'America/Santiago' para Chile, 'Europe/Madrid' para España, \
                 'Asia/Tokyo' para Japón, 'America/Mexico_City' para México, \
                 'America/Argentina/Buenos_Aires' para Argentina."
            }
        };
        serde_json::json!({
            "type": "object",
            "properties": {
                "timezone": {
                    "type": "string",
                    "description": description,
                }
            },
            "required": ["timezone"],
        })
    }

    async fn execute(&self, arguments_json: &str) -> Result<String, ToolError> {
        let timezone_name = parse_timezone_argument(arguments_json, &self.lang)?;
        let tz: Tz = timezone_name.parse().map_err(|_| ToolError::InvalidArguments(invalid_timezone_message(&timezone_name, &self.lang)))?;
        Ok(format_current_time_at(Utc::now(), tz))
    }
}

fn invalid_timezone_message(timezone_name: &str, lang: &str) -> String {
    match lang {
        "en" => format!(
            "'{timezone_name}' is not a valid IANA timezone identifier. Use the \
             'Continent/City' format, e.g. 'America/Santiago', 'Europe/Madrid', 'Asia/Tokyo'."
        ),
        "pt" => format!(
            "'{timezone_name}' não é um identificador de fuso horário IANA válido. Use o \
             formato 'Continente/Cidade', ex. 'America/Santiago', 'Europe/Madrid', 'Asia/Tokyo'."
        ),
        "ja" => format!(
            "'{timezone_name}' は有効なIANAタイムゾーン識別子ではない。「大陸/都市」形式を使うこと。\
             例: 'America/Santiago'、'Europe/Madrid'、'Asia/Tokyo'。"
        ),
        "ko" => format!(
            "'{timezone_name}'은(는) 유효한 IANA 시간대 식별자가 아니다. '대륙/도시' 형식을 사용할 것. \
             예: 'America/Santiago', 'Europe/Madrid', 'Asia/Tokyo'."
        ),
        "zh" => format!(
            "'{timezone_name}' 不是有效的 IANA 时区标识符。请使用“大洲/城市”格式，例如 \
             'America/Santiago'、'Europe/Madrid'、'Asia/Tokyo'。"
        ),
        _ => format!(
            "'{timezone_name}' no es un identificador de zona horaria IANA válido. \
             Usá el formato 'Continente/Ciudad', ej. 'America/Santiago', \
             'Europe/Madrid', 'Asia/Tokyo'."
        ),
    }
}

fn parse_timezone_argument(arguments_json: &str, lang: &str) -> Result<String, ToolError> {
    let value: serde_json::Value =
        serde_json::from_str(arguments_json).map_err(|err| ToolError::InvalidArguments(err.to_string()))?;
    value
        .get("timezone")
        .and_then(|q| q.as_str())
        .filter(|q| !q.trim().is_empty())
        .map(str::to_string)
        .ok_or_else(|| ToolError::InvalidArguments(missing_timezone_message(lang)))
}

fn missing_timezone_message(lang: &str) -> String {
    match lang {
        "en" => "missing 'timezone' field".to_string(),
        "pt" => "falta o campo 'timezone'".to_string(),
        "ja" => "'timezone' フィールドがありません".to_string(),
        "ko" => "'timezone' 필드가 없습니다".to_string(),
        "zh" => "缺少 'timezone' 字段".to_string(),
        _ => "falta el campo 'timezone'".to_string(),
    }
}

/// Split out from `execute` so a test can pin `now` instead of racing the
/// real clock — direct, deterministic computation, no HTTP involved either
/// way (unlike `brave.rs`'s pure-parsing-only split, this whole tool never
/// touches the network).
fn format_current_time_at(now: DateTime<Utc>, tz: Tz) -> String {
    let local = now.with_timezone(&tz);
    format!("{} ({})", local.format("%Y-%m-%d %H:%M:%S %:z"), tz.name())
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    #[test]
    fn parses_the_timezone_argument() {
        assert_eq!(parse_timezone_argument(r#"{"timezone":"America/Santiago"}"#, "es").unwrap(), "America/Santiago");
    }

    #[test]
    fn rejects_missing_timezone() {
        assert!(matches!(parse_timezone_argument("{}", "es"), Err(ToolError::InvalidArguments(_))));
    }

    #[test]
    fn rejects_malformed_json() {
        assert!(matches!(parse_timezone_argument("not json", "es"), Err(ToolError::InvalidArguments(_))));
    }

    #[test]
    fn formats_a_fixed_instant_in_a_non_dst_timezone() {
        // Asia/Tokyo (JST, UTC+9) never observes DST, so this is exact
        // year-round — no ambiguity from a timezone database that changes.
        let now = Utc.with_ymd_and_hms(2026, 9, 6, 10, 30, 0).unwrap();
        let text = format_current_time_at(now, chrono_tz::Asia::Tokyo);
        assert_eq!(text, "2026-09-06 19:30:00 +09:00 (Asia/Tokyo)");
    }

    #[tokio::test]
    async fn returns_the_current_time_for_a_valid_timezone() {
        let tool = CurrentTimeTool::new("es");
        let result = tool.execute(r#"{"timezone":"America/Santiago"}"#).await.unwrap();
        assert!(result.contains("America/Santiago"));
    }

    #[tokio::test]
    async fn rejects_a_bare_country_name_instead_of_an_iana_identifier() {
        let tool = CurrentTimeTool::new("es");
        let result = tool.execute(r#"{"timezone":"Chile"}"#).await;
        assert!(matches!(result, Err(ToolError::InvalidArguments(_))));
    }

    #[tokio::test]
    async fn rejects_missing_timezone_argument() {
        let tool = CurrentTimeTool::new("es");
        let result = tool.execute("{}").await;
        assert!(matches!(result, Err(ToolError::InvalidArguments(_))));
    }

    #[tokio::test]
    async fn accepts_the_legacy_chile_continental_alias() {
        // The model sometimes produces this instead of `America/Santiago`
        // — it's a real (if deprecated) IANA backward-compatibility link
        // that `chrono-tz` resolves correctly, confirmed live: not every
        // non-`Continente/Ciudad`-looking name the model guesses is
        // actually invalid.
        let tool = CurrentTimeTool::new("es");
        let result = tool.execute(r#"{"timezone":"Chile/Continental"}"#).await.unwrap();
        assert!(result.contains("Chile/Continental"));
    }

    #[test]
    fn description_and_schema_follow_the_configured_language() {
        let tool = CurrentTimeTool::new("en");
        assert!(tool.description().contains("ANOTHER country"));
        assert!(!tool.description().contains("OTRO país"));
        let schema = tool.parameters_schema();
        assert!(schema["properties"]["timezone"]["description"].as_str().unwrap().contains("IANA timezone"));
    }

    #[test]
    fn unknown_language_falls_back_to_spanish() {
        let tool = CurrentTimeTool::new("fr");
        assert!(tool.description().contains("OTRO país"));
    }
}
