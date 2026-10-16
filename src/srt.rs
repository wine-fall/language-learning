//! SRT subtitle parsing.
//!
//! Sentences ("cues") are identified by their 1-based position in the parsed
//! file, not by the counter written in the SRT. In well-formed files the two
//! are the same; `listen cues <file.srt>` prints the numbering the app uses.

/// One subtitle entry. Times are in milliseconds from the start of the audio.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cue {
    pub start_ms: u64,
    pub end_ms: u64,
    /// Subtitle text with formatting tags removed; lines joined by `\n`.
    pub text: String,
}

impl Cue {
    /// Text on a single line, for dictation and compact display.
    pub fn one_line(&self) -> String {
        self.text.split('\n').collect::<Vec<_>>().join(" ")
    }
}

/// Parses SRT text. Blocks without a valid timing line or without any text
/// are skipped. The result is sorted by start time.
pub fn parse(input: &str) -> Vec<Cue> {
    let input = input.strip_prefix('\u{feff}').unwrap_or(input);
    let mut cues = Vec::new();
    let mut block: Vec<&str> = Vec::new();
    for line in input.lines().chain(std::iter::once("")) {
        let line = line.trim_end_matches('\r');
        if line.trim().is_empty() {
            if let Some(cue) = parse_block(&block) {
                cues.push(cue);
            }
            block.clear();
        } else {
            block.push(line);
        }
    }
    cues.sort_by_key(|c| c.start_ms);
    cues
}

fn parse_block(lines: &[&str]) -> Option<Cue> {
    let timing = lines.iter().position(|l| l.contains("-->"))?;
    let (start_ms, end_ms) = parse_timing(lines[timing])?;
    let text: Vec<String> = lines[timing + 1..]
        .iter()
        .map(|l| strip_tags(l).trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();
    if text.is_empty() {
        return None;
    }
    Some(Cue {
        start_ms,
        end_ms: end_ms.max(start_ms),
        text: text.join("\n"),
    })
}

fn parse_timing(line: &str) -> Option<(u64, u64)> {
    let (a, b) = line.split_once("-->")?;
    // Anything after the end time (e.g. position hints) is ignored.
    let b = b.split_whitespace().next()?;
    Some((parse_time(a.trim())?, parse_time(b)?))
}

/// Parses `HH:MM:SS,mmm` (a `.` separator and missing hours are accepted).
pub fn parse_time(s: &str) -> Option<u64> {
    let s = s.replace('.', ",");
    let (hms, ms) = s.split_once(',').unwrap_or((&s, "0"));
    let parts: Vec<&str> = hms.split(':').collect();
    let (h, m, sec) = match parts.as_slice() {
        [h, m, s] => (*h, *m, *s),
        [m, s] => ("0", *m, *s),
        _ => return None,
    };
    let num = |x: &str| x.trim().parse::<u64>().ok();
    let ms_digits: String = ms.chars().take(3).collect();
    let mut millis = num(&ms_digits)?;
    for _ in ms_digits.len()..3 {
        millis *= 10;
    }
    Some(((num(h)? * 60 + num(m)?) * 60 + num(sec)?) * 1000 + millis)
}

/// Removes `<i>`-style HTML tags and `{\an8}`-style override blocks.
fn strip_tags(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut depth: Option<char> = None;
    for ch in line.chars() {
        match (depth, ch) {
            (None, '<') => depth = Some('>'),
            (None, '{') => depth = Some('}'),
            (Some(close), c) if c == close => depth = None,
            (Some(_), _) => {}
            (None, c) => out.push(c),
        }
    }
    out
}

/// Formats milliseconds as `M:SS` (or `H:MM:SS` past an hour).
pub fn fmt_time(ms: u64) -> String {
    let s = ms / 1000;
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\u{feff}1\r\n00:00:01,000 --> 00:00:02,500\r\nHello there.\r\n\r\n\
2\r\n00:00:03,000 --> 00:00:04,000 X1:10 X2:20\r\n<i>How are</i> you?\r\n- Fine.\r\n\r\n\
3\r\nnot a timing line\r\nskipped\r\n\r\n\
4\r\n00:00:05.5 --> 00:00:06,250\r\n{\\an8}It's me.\r\n";

    #[test]
    fn parses_blocks_crlf_bom_and_tags() {
        let cues = parse(SAMPLE);
        assert_eq!(cues.len(), 3);
        assert_eq!(cues[0].start_ms, 1000);
        assert_eq!(cues[0].end_ms, 2500);
        assert_eq!(cues[0].text, "Hello there.");
        assert_eq!(cues[1].text, "How are you?\n- Fine.");
        assert_eq!(cues[1].one_line(), "How are you? - Fine.");
        assert_eq!(cues[2].start_ms, 5500);
        assert_eq!(cues[2].text, "It's me.");
    }

    #[test]
    fn skips_empty_text_and_sorts() {
        let src = "1\n00:00:09,000 --> 00:00:10,000\nlater\n\n\
2\n00:00:01,000 --> 00:00:02,000\n<i></i>\n\n\
3\n00:00:03,000 --> 00:00:04,000\nearlier\n";
        let cues = parse(src);
        let texts: Vec<_> = cues.iter().map(|c| c.text.as_str()).collect();
        assert_eq!(texts, ["earlier", "later"]);
    }

    #[test]
    fn time_formats() {
        assert_eq!(parse_time("01:02:03,004"), Some(3_723_004));
        assert_eq!(parse_time("02:03.5"), Some(123_500));
        assert_eq!(parse_time("bad"), None);
        assert_eq!(fmt_time(65_000), "1:05");
        assert_eq!(fmt_time(3_723_004), "1:02:03");
    }
}
