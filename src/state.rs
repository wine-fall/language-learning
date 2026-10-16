//! Persistent learning state: `<data dir>/language-learning/state.json`.
//!
//! `LL_HOME`, when set, replaces the platform data directory (used by tests).

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::leitner::Schedule;

pub const STATE_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub version: u32,
    /// Keyed by the episode's audio path (absolute when it could be resolved).
    #[serde(default)]
    pub episodes: BTreeMap<String, Episode>,
    #[serde(default)]
    pub cards: Vec<Card>,
}

impl Default for State {
    fn default() -> Self {
        State {
            version: STATE_VERSION,
            episodes: BTreeMap::new(),
            cards: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Episode {
    /// Last playback position.
    #[serde(default)]
    pub position_ms: u64,
    /// Subtitle shift: positive means subtitles come later.
    #[serde(default)]
    pub offset_ms: i64,
    /// Per-sentence marks, keyed by 1-based sentence number.
    #[serde(default)]
    pub marks: BTreeMap<usize, Mark>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mark {
    /// 听懂
    Clear,
    /// 没听清
    Unclear,
}

/// A saved expression. `file` and `cue` locate the original sentence so the
/// review screen can replay its audio.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Card {
    pub file: String,
    pub cue: usize,
    pub expression: String,
    pub meaning: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// The sentence text, kept in case the subtitle file goes missing.
    pub sentence: String,
    pub added: i64,
    pub schedule: Schedule,
}

impl State {
    pub fn episode(&mut self, key: &str) -> &mut Episode {
        self.episodes.entry(key.to_string()).or_default()
    }

    pub fn find_card(&self, file: &str, cue: usize, expression: &str) -> Option<usize> {
        self.cards
            .iter()
            .position(|c| c.file == file && c.cue == cue && c.expression == expression)
    }

    /// Reads the state file; a missing file gives an empty state. A file that
    /// exists but cannot be parsed is an error, so it is never overwritten.
    pub fn load(path: &Path) -> Result<State, String> {
        match fs::read_to_string(path) {
            Ok(text) => serde_json::from_str(&text)
                .map_err(|e| format!("cannot parse {}: {e}", path.display())),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(State::default()),
            Err(e) => Err(format!("cannot read {}: {e}", path.display())),
        }
    }

    /// Writes atomically (temporary file + rename).
    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("json.tmp");
        let json = serde_json::to_string_pretty(self).map_err(io::Error::other)?;
        fs::write(&tmp, json + "\n")?;
        fs::rename(&tmp, path)
    }
}

/// Location of `state.json`, or `None` if no data directory is known.
pub fn state_path() -> Option<PathBuf> {
    let base = match std::env::var_os("LL_HOME") {
        Some(home) if !home.is_empty() => PathBuf::from(home),
        _ => dirs::data_dir()?,
    };
    Some(base.join("language-learning").join("state.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ll-state-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    fn sample() -> State {
        let mut s = State::default();
        let ep = s.episode("/media/ep01.m4a");
        ep.position_ms = 61_500;
        ep.offset_ms = -250;
        ep.marks.insert(3, Mark::Unclear);
        ep.marks.insert(4, Mark::Clear);
        s.cards.push(Card {
            file: "/media/ep01.m4a".into(),
            cue: 3,
            expression: "hang out".into(),
            meaning: "一起玩".into(),
            note: None,
            sentence: "Wanna hang out?".into(),
            added: 20_000,
            schedule: Schedule::new(20_000),
        });
        s
    }

    #[test]
    fn round_trip_and_missing_file() {
        let dir = temp_dir("rt");
        let path = dir.join("language-learning").join("state.json");
        assert_eq!(State::load(&path).unwrap(), State::default());
        let s = sample();
        s.save(&path).unwrap();
        assert_eq!(State::load(&path).unwrap(), s);
        assert!(!path.with_extension("json.tmp").exists());
        assert_eq!(s.find_card("/media/ep01.m4a", 3, "hang out"), Some(0));
        assert_eq!(s.find_card("/media/ep01.m4a", 4, "hang out"), None);
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("\"unclear\"") && text.contains("\"box\": 1"));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn corrupt_file_is_an_error() {
        let dir = temp_dir("bad");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("state.json");
        fs::write(&path, "{ not json").unwrap();
        assert!(State::load(&path).is_err());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn tolerates_missing_optional_fields() {
        let s: State = serde_json::from_str(r#"{"version":1,"episodes":{"a":{}}}"#).unwrap();
        assert_eq!(s.episodes["a"], Episode::default());
        assert!(s.cards.is_empty());
    }

    #[test]
    fn ll_home_overrides_data_dir() {
        // Only this test touches LL_HOME.
        let dir = temp_dir("home");
        unsafe { std::env::set_var("LL_HOME", &dir) };
        assert_eq!(
            state_path().unwrap(),
            dir.join("language-learning").join("state.json")
        );
        unsafe { std::env::remove_var("LL_HOME") };
    }
}
