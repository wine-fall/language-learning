//! `<name>.words.json`: key expressions with Chinese glosses, plus optional
//! scene boundaries. See the README for the schema.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct WordsFile {
    #[serde(default)]
    pub items: Vec<Item>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scenes: Vec<SceneMark>,
}

/// A key expression found in one sentence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Item {
    /// 1-based sentence number (see `listen cues`).
    pub cue: usize,
    /// The expression as it appears in the sentence.
    pub text: String,
    /// Short Chinese gloss.
    pub meaning: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// A scene starts at sentence `start` and runs until the next scene starts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SceneMark {
    pub start: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

/// Most expressions shown on a scene's preview page.
pub const PREVIEW_MAX: usize = 8;

impl WordsFile {
    pub fn parse(json: &str) -> Result<Self, String> {
        serde_json::from_str(json).map_err(|e| e.to_string())
    }

    /// Items for sentence `cue` (1-based), in file order. Keys 1-9 index this list.
    pub fn for_cue(&self, cue: usize) -> Vec<&Item> {
        self.items.iter().filter(|i| i.cue == cue).collect()
    }

    /// Up to [`PREVIEW_MAX`] items for sentences `first..=last`, in sentence
    /// order. When there are more, multi-word phrases are preferred over
    /// single words, then earlier sentences.
    pub fn preview(&self, first: usize, last: usize) -> Vec<&Item> {
        let mut items: Vec<&Item> = self
            .items
            .iter()
            .filter(|i| (first..=last).contains(&i.cue))
            .collect();
        if items.len() > PREVIEW_MAX {
            let mut ranked = items.clone();
            ranked.sort_by_key(|i| (!i.text.trim().contains(' '), i.cue));
            ranked.truncate(PREVIEW_MAX);
            items.retain(|i| ranked.iter().any(|r| std::ptr::eq(*r, *i)));
        }
        items
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(cue: usize, text: &str) -> Item {
        Item {
            cue,
            text: text.into(),
            meaning: "释义".into(),
            note: None,
        }
    }

    #[test]
    fn parses_minimal_and_full() {
        let w = WordsFile::parse(r#"{"items":[{"cue":2,"text":"hang out","meaning":"一起玩"}]}"#)
            .unwrap();
        assert_eq!(w.items[0].cue, 2);
        assert!(w.scenes.is_empty());

        let w = WordsFile::parse(
            r#"{"items":[{"cue":1,"text":"x","meaning":"y","note":"z"}],
                "scenes":[{"start":1,"title":"Opening"},{"start":30}]}"#,
        )
        .unwrap();
        assert_eq!(w.items[0].note.as_deref(), Some("z"));
        assert_eq!(w.scenes[1].start, 30);
        assert!(WordsFile::parse(r#"{"items":[{"cue":1}]}"#).is_err());
    }

    #[test]
    fn for_cue_keeps_file_order() {
        let w = WordsFile {
            items: vec![item(3, "b"), item(2, "x"), item(3, "a")],
            scenes: vec![],
        };
        let t: Vec<_> = w.for_cue(3).iter().map(|i| i.text.as_str()).collect();
        assert_eq!(t, ["b", "a"]);
    }

    #[test]
    fn preview_prefers_phrases_and_caps() {
        let mut items: Vec<Item> = (1..=10).map(|n| item(n, "word")).collect();
        items.push(item(9, "pick up"));
        items.push(item(10, "come on"));
        items.push(item(40, "out of range"));
        let w = WordsFile {
            items,
            scenes: vec![],
        };
        let p = w.preview(1, 10);
        assert_eq!(p.len(), PREVIEW_MAX);
        assert!(p.iter().any(|i| i.text == "pick up"));
        assert!(p.iter().any(|i| i.text == "come on"));
        assert!(p.windows(2).all(|w| w[0].cue <= w[1].cue));
        assert_eq!(w.preview(30, 50).len(), 1);
    }
}
