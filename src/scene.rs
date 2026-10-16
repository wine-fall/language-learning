//! Splitting an episode into scenes.

use crate::srt::Cue;
use crate::words::SceneMark;

/// A gap at least this long between two subtitles suggests a scene change.
pub const SCENE_GAP_MS: u64 = 4000;
pub const MIN_SCENE: usize = 20;
pub const MAX_SCENE: usize = 50;

/// A run of sentences: indices `start..end` into the cue list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scene {
    pub start: usize,
    pub end: usize,
    pub title: Option<String>,
}

impl Scene {
    pub fn len(&self) -> usize {
        self.end - self.start
    }

    pub fn contains(&self, idx: usize) -> bool {
        (self.start..self.end).contains(&idx)
    }
}

/// Default split: scenes of 20-50 sentences, cut at the first subtitle gap of
/// at least 4 s once the scene is long enough. Without such a gap the largest
/// gap in the allowed range is used. Fewer than 20 sentences form one scene.
pub fn split_by_gaps(cues: &[Cue]) -> Vec<Scene> {
    let n = cues.len();
    let gap = |i: usize| cues[i].start_ms.saturating_sub(cues[i - 1].end_ms);
    let mut scenes = Vec::new();
    let mut start = 0;
    while start < n {
        let end = if n - start <= MAX_SCENE {
            n
        } else {
            // `end` is the first index of the next scene. Keep at least
            // MIN_SCENE for whatever follows so the last scene is not tiny.
            let lo = start + MIN_SCENE;
            let hi = (start + MAX_SCENE).min(n - MIN_SCENE);
            (lo..=hi)
                .find(|&e| gap(e) >= SCENE_GAP_MS)
                .unwrap_or_else(|| (lo..=hi).max_by_key(|&e| gap(e)).unwrap_or(hi))
        };
        scenes.push(Scene {
            start,
            end,
            title: None,
        });
        start = end;
    }
    scenes
}

/// Scenes from `words.json`: each mark starts a scene (1-based sentence
/// number) that runs until the next mark. Sentences before the first mark
/// belong to the first scene; out-of-range or duplicate marks are ignored.
pub fn from_marks(n: usize, marks: &[SceneMark]) -> Vec<Scene> {
    let mut marks: Vec<&SceneMark> = marks
        .iter()
        .filter(|m| m.start >= 1 && m.start <= n)
        .collect();
    marks.sort_by_key(|m| m.start);
    marks.dedup_by_key(|m| m.start);
    let mut scenes: Vec<Scene> = Vec::new();
    for (i, m) in marks.iter().enumerate() {
        let start = if i == 0 { 0 } else { m.start - 1 };
        let end = marks.get(i + 1).map_or(n, |next| next.start - 1);
        scenes.push(Scene {
            start,
            end,
            title: m.title.clone(),
        });
    }
    scenes
}

/// Uses the marks when there are any, otherwise the default split.
pub fn scenes_for(cues: &[Cue], marks: &[SceneMark]) -> Vec<Scene> {
    let scenes = from_marks(cues.len(), marks);
    if scenes.is_empty() {
        split_by_gaps(cues)
    } else {
        scenes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Cues of 1 s each, 0.5 s apart, with a long gap before each index in `breaks`.
    fn cues(n: usize, breaks: &[usize]) -> Vec<Cue> {
        let mut t = 0;
        (0..n)
            .map(|i| {
                t += if breaks.contains(&i) { 10_000 } else { 500 };
                let c = Cue {
                    start_ms: t,
                    end_ms: t + 1000,
                    text: format!("line {i}"),
                };
                t += 1000;
                c
            })
            .collect()
    }

    fn sizes(s: &[Scene]) -> Vec<usize> {
        s.iter().map(Scene::len).collect()
    }

    #[test]
    fn cuts_at_natural_gaps() {
        let s = split_by_gaps(&cues(90, &[25, 60]));
        assert_eq!(sizes(&s), [25, 35, 30]);
        assert_eq!(s[1].start, 25);
        assert_eq!(s.last().unwrap().end, 90);
    }

    #[test]
    fn ignores_gaps_that_would_make_short_scenes() {
        // Gap at 5 is too early; the gap at 30 is used instead.
        let s = split_by_gaps(&cues(60, &[5, 30]));
        assert_eq!(sizes(&s), [30, 30]);
    }

    #[test]
    fn forces_cut_without_gaps_and_respects_limits() {
        let s = split_by_gaps(&cues(130, &[]));
        assert!(s.iter().all(|x| (MIN_SCENE..=MAX_SCENE).contains(&x.len())));
        assert_eq!(sizes(&s).iter().sum::<usize>(), 130);
        // A gap that would leave fewer than MIN_SCENE at the end is skipped.
        let s = split_by_gaps(&cues(60, &[45]));
        assert!(s.iter().all(|x| x.len() >= MIN_SCENE), "{:?}", sizes(&s));
    }

    #[test]
    fn short_episode_is_one_scene() {
        assert_eq!(sizes(&split_by_gaps(&cues(12, &[4]))), [12]);
        assert!(split_by_gaps(&[]).is_empty());
    }

    #[test]
    fn marks_override_default() {
        let marks = vec![
            SceneMark {
                start: 5,
                title: Some("Two".into()),
            },
            SceneMark {
                start: 1,
                title: None,
            },
            SceneMark {
                start: 99,
                title: None,
            },
        ];
        let s = scenes_for(&cues(10, &[]), &marks);
        assert_eq!(sizes(&s), [4, 6]);
        assert_eq!(s[1].title.as_deref(), Some("Two"));

        // First mark after sentence 1: earlier sentences join the first scene.
        let s = from_marks(10, &marks[..1]);
        assert_eq!((s[0].start, s[0].end), (0, 10));
        assert_eq!(sizes(&scenes_for(&cues(30, &[]), &[])), [30]);
    }
}
