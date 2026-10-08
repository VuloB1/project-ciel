/// Extracts complete `data: ...` payloads from a growing SSE text buffer,
/// removing consumed lines from `buffer` as it goes. Any trailing partial
/// line (split across a chunk boundary) is left in `buffer` for the next
/// call. Non-`data:` lines (blank separators, `event:`/`:` comment lines
/// used for keep-alives) are ignored — OpenAI-compatible SSE only ever
/// needs the data payloads.
///
/// Pulled out as a pure function specifically so it can be unit tested
/// without any networking involved — the actual streaming glue
/// (`openai_compatible.rs`) just calls this on whatever bytes arrive.
pub(crate) fn drain_sse_data_lines(buffer: &mut String) -> Vec<String> {
    let mut out = Vec::new();

    while let Some(pos) = buffer.find('\n') {
        let line: String = buffer.drain(..=pos).collect();
        let line = line.trim();
        if let Some(data) = line.strip_prefix("data:") {
            let data = data.trim();
            if !data.is_empty() {
                out.push(data.to_string());
            }
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_a_single_complete_line() {
        let mut buffer = "data: {\"a\":1}\n".to_string();
        assert_eq!(drain_sse_data_lines(&mut buffer), vec!["{\"a\":1}"]);
        assert!(buffer.is_empty());
    }

    #[test]
    fn leaves_a_partial_trailing_line_in_the_buffer() {
        let mut buffer = "data: {\"a\":1}\ndata: {\"a\":2".to_string();
        assert_eq!(drain_sse_data_lines(&mut buffer), vec!["{\"a\":1}"]);
        assert_eq!(buffer, "data: {\"a\":2");

        buffer.push_str("}\n");
        assert_eq!(drain_sse_data_lines(&mut buffer), vec!["{\"a\":2}"]);
    }

    #[test]
    fn ignores_blank_lines_and_comments() {
        let mut buffer = "\n: keep-alive\n\ndata: {\"a\":1}\n\n".to_string();
        assert_eq!(drain_sse_data_lines(&mut buffer), vec!["{\"a\":1}"]);
    }

    #[test]
    fn ignores_event_lines() {
        let mut buffer = "event: message\ndata: {\"a\":1}\n".to_string();
        assert_eq!(drain_sse_data_lines(&mut buffer), vec!["{\"a\":1}"]);
    }

    #[test]
    fn handles_the_done_sentinel_like_any_other_payload() {
        let mut buffer = "data: [DONE]\n".to_string();
        assert_eq!(drain_sse_data_lines(&mut buffer), vec!["[DONE]"]);
    }

    #[test]
    fn empty_buffer_yields_nothing() {
        let mut buffer = String::new();
        assert!(drain_sse_data_lines(&mut buffer).is_empty());
    }

    #[test]
    fn multiple_events_in_one_chunk_all_extracted_in_order() {
        let mut buffer = "data: {\"a\":1}\ndata: {\"a\":2}\ndata: {\"a\":3}\n".to_string();
        assert_eq!(
            drain_sse_data_lines(&mut buffer),
            vec!["{\"a\":1}".to_string(), "{\"a\":2}".to_string(), "{\"a\":3}".to_string()]
        );
    }
}
