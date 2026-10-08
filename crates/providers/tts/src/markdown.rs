use std::sync::LazyLock;

use regex::Regex;

/// One ordered find-and-replace pass — same shape as
/// `aivt_core::redact_secrets`'s `RedactionRule`, applied in sequence
/// rather than as a single combined pattern so each construct can be
/// reasoned about (and tested) independently.
struct StripRule {
    pattern: Regex,
    replacement: &'static str,
}

// Order matters:
// - Fenced code blocks first, so nothing inside one (a list marker, a
//   heading-looking comment line) gets misread as real Markdown structure.
// - Links before the bare-URL pass, so a `[text](url)` becomes `text`
//   instead of `text ` + a dangling URL the bare-URL rule then removes.
// - Bold before italic: a naive single-`*`/`_` pass run first would treat
//   `**bold**` as two adjacent (and malformed) italic spans.
static RULES: LazyLock<Vec<StripRule>> = LazyLock::new(|| {
    vec![
        // ```code block``` (spans lines) -> removed entirely; reading a
        // code/JSON blob aloud verbatim helps no one.
        StripRule { pattern: Regex::new(r"(?s)```.*?```").expect("valid regex"), replacement: "" },
        // [text](url) -> text
        StripRule { pattern: Regex::new(r"\[([^\]]*)\]\([^)]*\)").expect("valid regex"), replacement: "$1" },
        // Bare URL -> removed; spelling out "h-t-t-p-s colon slash slash…" is worse than silence.
        StripRule { pattern: Regex::new(r"https?://\S+").expect("valid regex"), replacement: "" },
        // # Heading -> Heading
        StripRule { pattern: Regex::new(r"(?m)^#{1,6}\s*").expect("valid regex"), replacement: "" },
        // > quote -> quote
        StripRule { pattern: Regex::new(r"(?m)^\s*>+\s?").expect("valid regex"), replacement: "" },
        // "- item" / "* item" / "1. item" -> item
        StripRule { pattern: Regex::new(r"(?m)^\s*(?:[-*+]|\d+\.)\s+").expect("valid regex"), replacement: "" },
        // Horizontal rule ("---", "***", "___") -> removed. Spelled out per
        // character rather than with a backreference — the `regex` crate
        // doesn't support those (it guarantees linear-time matching).
        StripRule { pattern: Regex::new(r"(?m)^\s*(?:-{3,}|\*{3,}|_{3,})\s*$").expect("valid regex"), replacement: "" },
        // ~~text~~ -> text
        StripRule { pattern: Regex::new(r"~~(.+?)~~").expect("valid regex"), replacement: "$1" },
        // `code` -> code
        StripRule { pattern: Regex::new(r"`([^`]*)`").expect("valid regex"), replacement: "$1" },
        // **bold** -> bold. Deliberately no `__bold__` rule: chat models
        // observed in practice only ever use `**`, and a double-underscore
        // pattern is indistinguishable by regex alone from a real dunder
        // identifier (`__init__.py`, `__main__`) without a full parser —
        // better to leave a rare, never-actually-seen `__bold__` unstripped
        // than mangle a fairly common one.
        StripRule { pattern: Regex::new(r"\*\*(.+?)\*\*").expect("valid regex"), replacement: "$1" },
        // *italic* -> italic
        StripRule { pattern: Regex::new(r"\*(.+?)\*").expect("valid regex"), replacement: "$1" },
        // _italic_ -> italic, but only outside a word (`\w` counts `_`
        // itself, unlike the plain `[A-Za-z0-9]` a first attempt at this
        // used) *and* only when the content itself doesn't start/end with
        // an underscore or space. That second part is what a boundary
        // check alone can't catch: without it, `__init__.py` matches as
        // "_" + content "_init_" + "_" (the *inner* pair of its four
        // underscores), silently eating two of them. Real italic content
        // never starts/ends with `_` or whitespace, so this costs nothing
        // for genuine emphasis while leaving `mi_archivo_final.txt` and
        // `__init__.py` alone.
        StripRule {
            pattern: Regex::new(r"(^|\W)_([^_\s](?:.*?[^_\s])?)_(\W|$)").expect("valid regex"),
            replacement: "$1$2$3",
        },
        // Collapse the runs of spaces left behind by a removed link/URL/marker.
        StripRule { pattern: Regex::new(r"[ \t]{2,}").expect("valid regex"), replacement: " " },
    ]
});

/// Strips Markdown syntax and bare links — originally for handing text to a
/// speech engine (link/heading/list syntax read aloud verbatim "bracket
/// text bracket parenthesis you-are-el colon slash slash…" is worse than
/// just the plain words underneath), also reused by `aivt-service-twitch`
/// to keep `**bold**` and friends out of Twitch chat, which doesn't render
/// Markdown either. Deliberately not a full CommonMark parser — same "pure
/// function, hand-rolled, good enough for what a chat model actually
/// produces" style as `aivt_core::redact_secrets` or
/// `aivt_service_discord::policy::contains_word`.
pub fn strip_markdown(text: &str) -> String {
    let mut out = text.to_string();
    for rule in RULES.iter() {
        out = rule.pattern.replace_all(&out, rule.replacement).into_owned();
    }
    out.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_a_markdown_link_down_to_its_text() {
        assert_eq!(strip_markdown("mirá [VGen](https://vgen.co/ryonikku)"), "mirá VGen");
    }

    #[test]
    fn removes_a_bare_url() {
        assert_eq!(strip_markdown("andá a https://example.com/x/y ahora"), "andá a ahora");
    }

    #[test]
    fn strips_bold_and_italic_without_eating_unrelated_asterisks_or_underscores() {
        assert_eq!(strip_markdown("esto es **importante** y *urgente*"), "esto es importante y urgente");
        // Not emphasis at all — a paired-regex approach must leave these alone.
        assert_eq!(strip_markdown("5 * 3 es 15"), "5 * 3 es 15");
        assert_eq!(strip_markdown("mi_archivo_final.txt"), "mi_archivo_final.txt");
        assert_eq!(strip_markdown("corré __init__.py"), "corré __init__.py");
    }

    #[test]
    fn strips_real_underscore_emphasis_at_word_boundaries() {
        assert_eq!(strip_markdown("esto es _urgente_ ahora"), "esto es urgente ahora");
        assert_eq!(strip_markdown("_urgente_ ahora"), "urgente ahora");
        assert_eq!(strip_markdown("ahora _urgente_"), "ahora urgente");
    }

    #[test]
    fn strips_heading_markers() {
        assert_eq!(strip_markdown("# Servicios principales"), "Servicios principales");
    }

    #[test]
    fn strips_list_markers_per_line() {
        assert_eq!(strip_markdown("- uno\n- dos\n1. tres"), "uno\ndos\ntres");
    }

    #[test]
    fn strips_inline_code_and_strikethrough() {
        assert_eq!(strip_markdown("corré `cargo test` y ~~listo~~ hecho"), "corré cargo test y listo hecho");
    }

    #[test]
    fn strips_a_fenced_code_block_entirely() {
        let input = "mirá esto:\n```\nfn main() {}\n```\nfin";
        assert_eq!(strip_markdown(input), "mirá esto:\n\nfin");
    }

    #[test]
    fn a_realistic_combined_chat_message() {
        let input = "**Servicios principales**:\n- Comisiones y [portafolio](https://vgen.co/ryonikku) de arte\n- PNGtubers";
        assert_eq!(strip_markdown(input), "Servicios principales:\nComisiones y portafolio de arte\nPNGtubers");
    }
}
