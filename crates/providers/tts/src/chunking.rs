//! Helpers for engines that cap the text per request (Groq's Orpheus accepts
//! at most 200 characters): split a reply into pieces under the cap, and glue
//! the resulting WAV clips back into one so playback (and the optional saved
//! copy) stay a single audio.

/// Splits `text` into pieces of at most `max_chars` characters, preferring to
/// cut after sentence endings, then at spaces, and only as a last resort in
/// the middle of a word. Pieces are trimmed; empty ones are dropped.
pub fn split_text(text: &str, max_chars: usize) -> Vec<String> {
    let max_chars = max_chars.max(1);
    let mut pieces: Vec<String> = Vec::new();
    let mut current = String::new();

    for sentence in split_sentences(text) {
        let sentence_len = sentence.chars().count();
        let current_len = current.chars().count();

        if sentence_len > max_chars {
            // A single sentence over the cap: flush, then break it by words.
            push_trimmed(&mut pieces, std::mem::take(&mut current));
            split_by_words(&sentence, max_chars, &mut pieces);
        } else if current_len + sentence_len > max_chars {
            push_trimmed(&mut pieces, std::mem::take(&mut current));
            current = sentence;
        } else {
            current.push_str(&sentence);
        }
    }
    push_trimmed(&mut pieces, current);
    pieces
}

fn push_trimmed(pieces: &mut Vec<String>, piece: String) {
    let trimmed = piece.trim();
    if !trimmed.is_empty() {
        pieces.push(trimmed.to_string());
    }
}

/// Sentences keep their trailing punctuation and whitespace, so concatenating
/// them reproduces the input.
fn split_sentences(text: &str) -> Vec<String> {
    let mut sentences = Vec::new();
    let mut current = String::new();
    let mut after_end = false;
    for ch in text.chars() {
        if after_end && !ch.is_whitespace() {
            sentences.push(std::mem::take(&mut current));
            after_end = false;
        }
        current.push(ch);
        if matches!(ch, '.' | '!' | '?' | '…' | '\n') {
            after_end = true;
        }
    }
    if !current.is_empty() {
        sentences.push(current);
    }
    sentences
}

fn split_by_words(sentence: &str, max_chars: usize, pieces: &mut Vec<String>) {
    let mut current = String::new();
    for word in sentence.split_whitespace() {
        let word_len = word.chars().count();
        if word_len > max_chars {
            push_trimmed(pieces, std::mem::take(&mut current));
            let chars: Vec<char> = word.chars().collect();
            for part in chars.chunks(max_chars) {
                pieces.push(part.iter().collect());
            }
            continue;
        }
        let needed = if current.is_empty() { word_len } else { current.chars().count() + 1 + word_len };
        if needed > max_chars {
            push_trimmed(pieces, std::mem::take(&mut current));
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(word);
    }
    push_trimmed(pieces, current);
}

struct WavParts<'a> {
    fmt: &'a [u8],
    data: &'a [u8],
}

fn parse_wav(bytes: &[u8]) -> Result<WavParts<'_>, String> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err("el audio recibido no es un WAV válido".to_string());
    }
    let mut fmt = None;
    let mut data = None;
    let mut offset = 12;
    while offset + 8 <= bytes.len() {
        let id = &bytes[offset..offset + 4];
        let declared = u32::from_le_bytes([bytes[offset + 4], bytes[offset + 5], bytes[offset + 6], bytes[offset + 7]]) as usize;
        let start = offset + 8;
        // Streaming servers sometimes declare 0 / 0xFFFFFFFF for `data`:
        // anything that doesn't fit means "until the end of the buffer".
        let end = if start.saturating_add(declared) > bytes.len() || (id == b"data" && declared == 0) { bytes.len() } else { start + declared };
        match id {
            b"fmt " => fmt = Some(&bytes[start..end]),
            b"data" => {
                data = Some(&bytes[start..end]);
                break;
            }
            _ => {}
        }
        offset = end + (end % 2);
    }
    match (fmt, data) {
        (Some(fmt), Some(data)) => Ok(WavParts { fmt, data }),
        _ => Err("al WAV recibido le falta la cabecera `fmt` o los datos".to_string()),
    }
}

/// Joins WAV clips that share the same format (as consecutive responses of
/// one engine do) into a single WAV.
pub fn concat_wavs(clips: &[Vec<u8>]) -> Result<Vec<u8>, String> {
    let parsed: Vec<WavParts<'_>> = clips.iter().map(|clip| parse_wav(clip)).collect::<Result<_, _>>()?;
    let first = parsed.first().ok_or_else(|| "no hay audio que unir".to_string())?;
    if parsed.iter().any(|p| p.fmt != first.fmt) {
        return Err("los fragmentos de audio tienen formatos distintos".to_string());
    }
    let data_len: usize = parsed.iter().map(|p| p.data.len()).sum();
    let fmt_len = first.fmt.len();
    let riff_len = 4 + (8 + fmt_len + fmt_len % 2) + (8 + data_len + data_len % 2);

    let mut out = Vec::with_capacity(riff_len + 8);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(riff_len as u32).to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&(fmt_len as u32).to_le_bytes());
    out.extend_from_slice(first.fmt);
    if fmt_len % 2 == 1 {
        out.push(0);
    }
    out.extend_from_slice(b"data");
    out.extend_from_slice(&(data_len as u32).to_le_bytes());
    for part in &parsed {
        out.extend_from_slice(part.data);
    }
    if data_len % 2 == 1 {
        out.push(0);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wav(samples: &[u8]) -> Vec<u8> {
        let fmt = [1u8, 0, 1, 0, 0x80, 0xBB, 0, 0, 0, 0x77, 1, 0, 2, 0, 16, 0];
        let mut out = Vec::new();
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&((4 + 8 + fmt.len() + 8 + samples.len()) as u32).to_le_bytes());
        out.extend_from_slice(b"WAVE");
        out.extend_from_slice(b"fmt ");
        out.extend_from_slice(&(fmt.len() as u32).to_le_bytes());
        out.extend_from_slice(&fmt);
        out.extend_from_slice(b"data");
        out.extend_from_slice(&(samples.len() as u32).to_le_bytes());
        out.extend_from_slice(samples);
        out
    }

    #[test]
    fn short_text_is_a_single_piece() {
        assert_eq!(split_text("Hola, ¿qué tal?", 200), vec!["Hola, ¿qué tal?"]);
    }

    #[test]
    fn splits_at_sentence_boundaries_without_exceeding_the_cap() {
        let pieces = split_text("Primera frase corta. Segunda frase corta. Tercera frase corta.", 45);
        assert_eq!(pieces, vec!["Primera frase corta. Segunda frase corta.", "Tercera frase corta."]);
        assert!(pieces.iter().all(|p| p.chars().count() <= 45));
    }

    #[test]
    fn a_sentence_longer_than_the_cap_is_split_by_words() {
        let text = "uno dos tres cuatro cinco seis siete ocho nueve diez";
        let pieces = split_text(text, 20);
        assert!(pieces.len() > 1);
        assert!(pieces.iter().all(|p| p.chars().count() <= 20));
        assert_eq!(pieces.join(" "), text);
    }

    #[test]
    fn a_word_longer_than_the_cap_is_hard_split() {
        let pieces = split_text("abcdefghij", 4);
        assert_eq!(pieces, vec!["abcd", "efgh", "ij"]);
    }

    #[test]
    fn counts_characters_not_bytes() {
        // 5 chars, 10 bytes: must fit a cap of 5.
        assert_eq!(split_text("ñññññ", 5), vec!["ñññññ"]);
    }

    #[test]
    fn empty_and_whitespace_only_text_gives_no_pieces() {
        assert!(split_text("   \n ", 200).is_empty());
    }

    #[test]
    fn concatenating_wavs_keeps_one_header_and_all_samples() {
        let joined = concat_wavs(&[wav(&[1, 2, 3, 4]), wav(&[5, 6])]).unwrap();
        let parts = parse_wav(&joined).unwrap();
        assert_eq!(parts.data, &[1, 2, 3, 4, 5, 6]);
        assert_eq!(parts.fmt.len(), 16);
        assert_eq!(&joined[0..4], b"RIFF");
    }

    #[test]
    fn a_data_size_of_zero_means_until_the_end() {
        let mut streamed = wav(&[9, 9, 9, 9]);
        let data_size_at = streamed.len() - 4 - 4;
        streamed[data_size_at..data_size_at + 4].copy_from_slice(&0u32.to_le_bytes());
        assert_eq!(parse_wav(&streamed).unwrap().data, &[9, 9, 9, 9]);
    }

    #[test]
    fn non_wav_input_is_rejected() {
        assert!(concat_wavs(&[b"ID3 not a wav".to_vec()]).is_err());
    }

    #[test]
    fn clips_with_different_formats_are_rejected() {
        let mut other = wav(&[1, 2]);
        other[22] = 2; // channels
        assert!(concat_wavs(&[wav(&[1, 2]), other]).is_err());
    }
}
