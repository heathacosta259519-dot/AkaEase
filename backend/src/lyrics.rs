use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricLine {
    pub time_ms: u64,
    pub text: String,
    pub translation: Option<String>,
}

fn timestamp(tag: &str) -> Option<u64> {
    let (minutes, rest) = tag.split_once(':')?;
    let (seconds, fraction) = rest
        .split_once('.')
        .or_else(|| rest.split_once(':'))
        .unwrap_or((rest, ""));
    if !minutes.bytes().all(|b| b.is_ascii_digit())
        || !seconds.bytes().all(|b| b.is_ascii_digit())
        || !fraction.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    let seconds = seconds.parse::<u64>().ok()?;
    if seconds >= 60 {
        return None;
    }
    let mut millis = 0;
    for (index, byte) in fraction.bytes().take(3).enumerate() {
        millis += u64::from(byte - b'0') * [100, 10, 1][index];
    }
    minutes
        .parse::<u64>()
        .ok()?
        .checked_mul(60_000)?
        .checked_add(seconds * 1000)?
        .checked_add(millis)
}

fn parse(source: &str) -> Vec<(u64, String)> {
    let offset = source
        .lines()
        .filter_map(|line| {
            line.trim()
                .strip_prefix("[offset:")?
                .strip_suffix(']')?
                .trim()
                .parse::<i64>()
                .ok()
        })
        .next_back()
        .unwrap_or(0);
    let mut lines = Vec::new();
    for line in source.lines() {
        let mut text = line.trim();
        let mut times = Vec::new();
        while let Some(rest) = text.strip_prefix('[') {
            let Some((tag, tail)) = rest.split_once(']') else {
                break;
            };
            if let Some(time) = timestamp(tag) {
                times.push(time.saturating_add_signed(offset));
            }
            text = tail;
        }
        for time in times {
            lines.push((time, text.trim().to_owned()));
        }
    }
    lines.sort_by_key(|line| line.0);
    lines
}

pub fn parse_lyrics(original: &str, translated: &str) -> Vec<LyricLine> {
    let mut translations: BTreeMap<u64, Vec<String>> = BTreeMap::new();
    for (time, text) in parse(translated) {
        translations.entry(time).or_default().push(text);
    }
    parse(original)
        .into_iter()
        .map(|(time_ms, text)| LyricLine {
            time_ms,
            text,
            translation: translations.get(&time_ms).map(|values| values.join("\n")),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn expands_tags_and_aligns_translation_with_offset() {
        let lines = parse_lyrics(
            "[offset:-100]\n[00:02.50][00:01.005]音楽\n[00:99.0]bad\n[ar:someone]",
            "[00:02.400]音乐",
        );
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].time_ms, 905);
        assert_eq!(lines[1].time_ms, 2400);
        assert_eq!(lines[1].translation.as_deref(), Some("音乐"));
    }
    #[test]
    fn rejects_overflow_and_handles_fraction_lengths() {
        assert_eq!(timestamp("999999999999999999999:01"), None);
        assert_eq!(timestamp("00:01.9"), Some(1900));
        assert_eq!(timestamp("00:01:9876"), Some(1987));
        assert_eq!(parse_lyrics("[offset:-2000]\n[00:01]x", "")[0].time_ms, 0);
    }
}
