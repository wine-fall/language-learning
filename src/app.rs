//! Play mode: the per-scene study flow
//! (preview → blind listening → sentence by sentence → relisten → final run).

use std::path::PathBuf;

use ratatui::crossterm::event::KeyCode;

use crate::dictation::{self, Diff};
use crate::leitner::{self, Schedule};
use crate::player::Player;
use crate::scene::Scene;
use crate::srt::Cue;
use crate::state::{Card, Mark, State};
use crate::words::{Item, WordsFile};

/// Extra audio after a sentence's end time so the last word is not clipped.
pub const TAIL_MS: u64 = 150;
/// One press of `[` or `]`.
pub const OFFSET_STEP_MS: i64 = 250;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// ① 预习: key expressions with Chinese glosses.
    Preview,
    /// ② 盲听: the whole scene without subtitles.
    Blind,
    /// ③ 逐句精听: one sentence at a time, dictation, marking.
    Intensive,
    /// ④ 复听: replay only the sentences marked unclear.
    Relisten,
    /// ⑤ 无字幕连播: the whole scene once more without subtitles.
    Final,
}

impl Stage {
    pub const ALL: [Stage; 5] = [
        Stage::Preview,
        Stage::Blind,
        Stage::Intensive,
        Stage::Relisten,
        Stage::Final,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Stage::Preview => "预习",
            Stage::Blind => "盲听",
            Stage::Intensive => "逐句精听",
            Stage::Relisten => "复听",
            Stage::Final => "无字幕连播",
        }
    }

    /// Circled step number, as shown in the stage bar and panel title.
    pub fn number(self) -> &'static str {
        match self {
            Stage::Preview => "①",
            Stage::Blind => "②",
            Stage::Intensive => "③",
            Stage::Relisten => "④",
            Stage::Final => "⑤",
        }
    }

    /// Number and label, e.g. "③ 逐句精听".
    pub fn title(self) -> String {
        format!("{} {}", self.number(), self.label())
    }

    /// Subtitles are never shown while listening to a whole scene.
    pub fn hides_subtitles(self) -> bool {
        matches!(self, Stage::Blind | Stage::Final)
    }
}

/// A finished dictation for one sentence.
pub struct Dictation {
    pub cue: usize,
    pub diff: Diff,
}

pub struct App {
    /// Shown in the title bar (file name only).
    pub title: String,
    /// State key and card source: the audio path.
    pub key: String,
    pub cues: Vec<Cue>,
    pub words: WordsFile,
    pub scenes: Vec<Scene>,
    pub scene: usize,
    pub stage: Stage,
    /// Current sentence (index into `cues`).
    pub cursor: usize,
    pub subtitles: bool,
    pub auto_pause: bool,
    /// Text being typed while in dictation input.
    pub input: Option<String>,
    pub dictation: Option<Dictation>,
    /// Sentences replayed in the relisten stage (snapshot taken on entry).
    pub relisten: Vec<usize>,
    pub show_help: bool,
    pub message: String,
    pub state: State,
    pub state_path: Option<PathBuf>,
    pub player: Box<dyn Player>,
    /// Sentences still to play in a relisten run.
    queue: Vec<usize>,
    was_playing: bool,
    pub quit: bool,
}

impl App {
    pub fn new(
        title: String,
        key: String,
        cues: Vec<Cue>,
        words: WordsFile,
        state: State,
        state_path: Option<PathBuf>,
        player: Box<dyn Player>,
    ) -> App {
        let scenes = crate::scene::scenes_for(&cues, &words.scenes);
        let mut app = App {
            title,
            key,
            cues,
            words,
            scenes,
            scene: 0,
            stage: Stage::Preview,
            cursor: 0,
            subtitles: false,
            auto_pause: true,
            input: None,
            dictation: None,
            relisten: Vec::new(),
            show_help: false,
            message: String::new(),
            state,
            state_path,
            player,
            queue: Vec::new(),
            was_playing: false,
            quit: false,
        };
        let pos = app
            .state
            .episodes
            .get(&app.key)
            .map_or(0, |e| e.position_ms);
        if pos > 0 {
            let idx = app.cue_at_any(pos);
            if let Some(s) = app.scenes.iter().position(|s| s.contains(idx)) {
                app.scene = s;
                app.message = format!("从上次的位置继续：场景 {}/{}", s + 1, app.scenes.len());
            }
        }
        app.cursor = app.current_scene().start;
        // Park the player at the scene so quitting from the preview keeps it.
        let start = app.scene_start_ms();
        app.player.seek(start);
        app
    }

    // ---- queries -------------------------------------------------------

    pub fn current_scene(&self) -> &Scene {
        &self.scenes[self.scene]
    }

    pub fn offset_ms(&self) -> i64 {
        self.state
            .episodes
            .get(&self.key)
            .map_or(0, |e| e.offset_ms)
    }

    pub fn cue_start(&self, i: usize) -> u64 {
        shift(self.cues[i].start_ms, self.offset_ms())
    }

    pub fn cue_end(&self, i: usize) -> u64 {
        shift(self.cues[i].end_ms, self.offset_ms()) + TAIL_MS
    }

    pub fn scene_start_ms(&self) -> u64 {
        self.cue_start(self.current_scene().start)
    }

    pub fn scene_end_ms(&self) -> u64 {
        self.cue_end(self.current_scene().end - 1)
    }

    /// The last sentence that has started at `pos`, anywhere in the episode.
    fn cue_at_any(&self, pos: u64) -> usize {
        let off = self.offset_ms();
        self.cues
            .partition_point(|c| shift(c.start_ms, off) <= pos)
            .saturating_sub(1)
    }

    /// Like `cue_at_any`, limited to the current scene.
    fn cue_at(&self, pos: u64) -> usize {
        let s = self.current_scene();
        self.cue_at_any(pos).clamp(s.start, s.end - 1)
    }

    pub fn mark(&self, i: usize) -> Option<Mark> {
        self.state
            .episodes
            .get(&self.key)
            .and_then(|e| e.marks.get(&(i + 1)).copied())
    }

    /// (marked, unclear) counts for the current scene.
    pub fn scene_marks(&self) -> (usize, usize) {
        let s = self.current_scene();
        let marks: Vec<_> = (s.start..s.end).filter_map(|i| self.mark(i)).collect();
        let unclear = marks.iter().filter(|m| **m == Mark::Unclear).count();
        (marks.len(), unclear)
    }

    pub fn items_for(&self, i: usize) -> Vec<&Item> {
        self.words.for_cue(i + 1)
    }

    pub fn preview_items(&self) -> Vec<&Item> {
        let s = self.current_scene();
        self.words.preview(s.start + 1, s.end)
    }

    pub fn is_saved(&self, item: &Item) -> bool {
        self.state
            .find_card(&self.key, item.cue, &item.text)
            .is_some()
    }

    /// Whether the current sentence's text may be shown.
    pub fn text_visible(&self) -> bool {
        !self.stage.hides_subtitles()
            && (self.subtitles
                || self
                    .dictation
                    .as_ref()
                    .is_some_and(|d| d.cue == self.cursor))
    }

    // ---- playback ------------------------------------------------------

    /// Plays one sentence and stops at its end.
    fn play_sentence(&mut self, i: usize) {
        self.set_cursor(i);
        let (start, end) = (self.cue_start(i), self.cue_end(i));
        self.player.play_range(start, Some(end));
        self.was_playing = true;
    }

    /// Plays from sentence `i` to the end of the scene.
    fn play_through(&mut self, i: usize) {
        self.set_cursor(i);
        let (start, end) = (self.cue_start(i), self.scene_end_ms());
        self.player.play_range(start, Some(end));
        self.was_playing = true;
    }

    /// Plays sentence `i` the way the current stage wants it.
    fn play_cue(&mut self, i: usize) {
        if self.stage == Stage::Intensive && !self.auto_pause {
            self.play_through(i);
        } else {
            self.play_sentence(i);
        }
    }

    fn pause(&mut self) {
        self.player.pause();
        self.was_playing = false;
        self.queue.clear();
    }

    fn set_cursor(&mut self, i: usize) {
        if i != self.cursor {
            self.dictation = None;
        }
        self.cursor = i;
    }

    /// Called about every 50 ms by the event loop.
    pub fn tick(&mut self) {
        self.player.tick();
        let playing = self.player.is_playing();
        let pos = self.player.position_ms();
        let follows = matches!(self.stage, Stage::Blind | Stage::Final)
            || (self.stage == Stage::Intensive && !self.auto_pause);
        if playing && follows {
            let i = self.cue_at(pos);
            self.set_cursor(i);
        }
        if self.was_playing && !playing {
            self.was_playing = false;
            self.finished();
        }
    }

    /// A requested range has played to its end.
    fn finished(&mut self) {
        self.message = match self.stage {
            Stage::Preview => String::new(),
            Stage::Blind => "盲听结束。Enter 进入逐句精听（r 再听一遍）".into(),
            Stage::Intensive if self.auto_pause => {
                "播完了：r 重播 · d 听写 · y 听懂 · m 没听清 · space 下一句".into()
            }
            Stage::Intensive => "场景播完。Enter 进入复听".into(),
            Stage::Relisten => {
                if self.queue.is_empty() {
                    "没听清的句子回放完毕。Enter 最后无字幕连播一遍".into()
                } else {
                    let next = self.queue.remove(0);
                    self.play_sentence(next);
                    return;
                }
            }
            Stage::Final => "本场景完成！Enter 进入下一场景".into(),
        };
    }

    // ---- stage changes -------------------------------------------------

    pub fn enter_stage(&mut self, stage: Stage) {
        self.pause();
        self.stage = stage;
        self.input = None;
        self.message.clear();
        let s = self.current_scene().clone();
        match stage {
            Stage::Preview => self.set_cursor(s.start),
            Stage::Blind | Stage::Final => self.play_through(s.start),
            Stage::Intensive => {
                let first = (s.start..s.end)
                    .find(|&i| self.mark(i).is_none())
                    .unwrap_or(s.start);
                self.play_cue(first);
            }
            Stage::Relisten => {
                self.relisten = (s.start..s.end)
                    .filter(|&i| self.mark(i) == Some(Mark::Unclear))
                    .collect();
                if self.relisten.is_empty() {
                    self.message = "这一场没有标记“没听清”的句子。Enter 最后无字幕连播一遍".into();
                } else {
                    self.message = format!("回放 {} 句没听清的句子", self.relisten.len());
                    self.play_relisten_from(0);
                }
            }
        }
    }

    fn play_relisten_from(&mut self, k: usize) {
        if let Some(&first) = self.relisten.get(k) {
            self.queue = self.relisten[k + 1..].to_vec();
            self.play_sentence(first);
        }
    }

    fn next_stage(&mut self) {
        let next = match self.stage {
            Stage::Preview => Stage::Blind,
            Stage::Blind => Stage::Intensive,
            Stage::Intensive => Stage::Relisten,
            Stage::Relisten => Stage::Final,
            Stage::Final => {
                if self.scene + 1 < self.scenes.len() {
                    self.goto_scene(self.scene + 1);
                } else {
                    self.pause();
                    self.message = "整集学完了！可以用 listen review 复习收藏的表达".into();
                }
                return;
            }
        };
        self.enter_stage(next);
    }

    pub fn goto_scene(&mut self, s: usize) {
        self.scene = s.min(self.scenes.len() - 1);
        self.enter_stage(Stage::Preview);
        let pos = self.scene_start_ms();
        self.player.seek(pos);
        self.save_position();
    }

    // ---- actions -------------------------------------------------------

    fn set_mark(&mut self, mark: Mark) {
        let cue = self.cursor + 1;
        self.state.episode(&self.key).marks.insert(cue, mark);
        self.save();
        if self.stage != Stage::Intensive {
            self.message = format!("#{cue} 标为{}", mark_label(mark));
            return;
        }
        let end = self.current_scene().end;
        if self.cursor + 1 < end {
            self.play_cue(self.cursor + 1);
            self.message = format!("#{cue} 标为{}", mark_label(mark));
        } else {
            self.pause();
            let (_, unclear) = self.scene_marks();
            self.message = format!("本场景逐句精听完成（没听清 {unclear} 句）。Enter 进入复听");
        }
    }

    /// Saves or unsaves the expression under number key `n` (1-based).
    fn toggle_card(&mut self, n: usize) {
        let items: Vec<Item> = if self.stage == Stage::Preview {
            self.preview_items().into_iter().cloned().collect()
        } else {
            self.items_for(self.cursor).into_iter().cloned().collect()
        };
        let Some(item) = items.get(n - 1) else {
            self.message = if items.is_empty() {
                "这里没有可收藏的重点表达".into()
            } else {
                format!("只有 {} 个重点表达", items.len())
            };
            return;
        };
        if let Some(idx) = self.state.find_card(&self.key, item.cue, &item.text) {
            self.state.cards.remove(idx);
            self.message = format!("已取消收藏「{}」", item.text);
        } else {
            let today = leitner::today();
            let sentence = self
                .cues
                .get(item.cue.wrapping_sub(1))
                .map(Cue::one_line)
                .unwrap_or_default();
            self.state.cards.push(Card {
                file: self.key.clone(),
                cue: item.cue,
                expression: item.text.clone(),
                meaning: item.meaning.clone(),
                ipa: item.ipa.clone(),
                note: item.note.clone(),
                sentence,
                added: today,
                schedule: Schedule::new(today),
            });
            self.message = format!("已收藏「{}」，明天开始复习", item.text);
        }
        self.save();
    }

    fn shift_offset(&mut self, delta: i64) {
        let ep = self.state.episode(&self.key);
        ep.offset_ms += delta;
        let off = ep.offset_ms;
        self.save();
        self.message = format!("字幕时间轴平移 {}", fmt_offset(off));
    }

    fn move_cursor(&mut self, forward: bool) {
        let s = self.current_scene().clone();
        match self.stage {
            Stage::Preview => {}
            Stage::Relisten => {
                if self.relisten.is_empty() {
                    return;
                }
                let k = self.relisten.iter().position(|&i| i == self.cursor);
                let k = match (k, forward) {
                    (Some(k), true) => (k + 1).min(self.relisten.len() - 1),
                    (Some(k), false) => k.saturating_sub(1),
                    (None, _) => 0,
                };
                self.pause();
                self.play_sentence(self.relisten[k]);
            }
            _ => {
                let i = if forward {
                    (self.cursor + 1).min(s.end - 1)
                } else {
                    self.cursor.saturating_sub(1).max(s.start)
                };
                if matches!(self.stage, Stage::Blind | Stage::Final) {
                    self.play_through(i);
                } else {
                    self.play_cue(i);
                }
            }
        }
    }

    fn space(&mut self) {
        if self.player.is_playing() {
            self.pause();
            self.message = "已暂停".into();
            return;
        }
        let pos = self.player.position_ms();
        match self.stage {
            Stage::Preview => self.next_stage(),
            Stage::Blind | Stage::Final => {
                if pos + 100 >= self.scene_end_ms() {
                    self.play_through(self.current_scene().start);
                } else {
                    self.player.resume();
                    self.was_playing = true;
                }
            }
            Stage::Intensive => {
                let in_sentence =
                    pos >= self.cue_start(self.cursor) && pos + 100 < self.cue_end(self.cursor);
                if in_sentence {
                    self.player.resume();
                    self.was_playing = true;
                } else if self.cursor + 1 < self.current_scene().end {
                    self.play_cue(self.cursor + 1);
                } else {
                    self.message = "已是本场景最后一句。Enter 进入复听".into();
                }
            }
            Stage::Relisten => {
                let k = self
                    .relisten
                    .iter()
                    .position(|&i| i == self.cursor)
                    .unwrap_or(0);
                self.play_relisten_from(k);
            }
        }
    }

    fn replay(&mut self) {
        match self.stage {
            Stage::Preview => {}
            Stage::Blind | Stage::Final => self.play_through(self.current_scene().start),
            Stage::Intensive | Stage::Relisten => {
                self.queue.clear();
                self.play_sentence(self.cursor);
            }
        }
    }

    fn toggle_auto_pause(&mut self) {
        self.auto_pause = !self.auto_pause;
        self.message = format!(
            "逐句自动暂停：{}",
            if self.auto_pause { "开" } else { "关" }
        );
        if self.stage == Stage::Intensive && self.player.is_playing() {
            let pos = self.player.position_ms();
            let end = if self.auto_pause {
                self.cue_end(self.cursor)
            } else {
                self.scene_end_ms()
            };
            self.player.play_range(pos, Some(end));
        }
    }

    // ---- input ---------------------------------------------------------

    pub fn on_key(&mut self, code: KeyCode) {
        if self.show_help {
            self.show_help = false;
            return;
        }
        if self.input.is_some() {
            self.on_input_key(code);
            return;
        }
        match code {
            KeyCode::Char('q') => self.quit(),
            KeyCode::Char('?') => self.show_help = true,
            KeyCode::Enter => self.next_stage(),
            KeyCode::Char(' ') => self.space(),
            KeyCode::Char('r') => self.replay(),
            KeyCode::Char('j') | KeyCode::Down | KeyCode::Right => self.move_cursor(true),
            KeyCode::Char('k') | KeyCode::Up | KeyCode::Left => self.move_cursor(false),
            KeyCode::Char('p') => self.toggle_auto_pause(),
            KeyCode::Char('c') => {
                self.subtitles = !self.subtitles;
                self.message = if self.stage.hides_subtitles() {
                    "这一阶段不显示字幕".into()
                } else {
                    format!("字幕：{}", if self.subtitles { "英文" } else { "关" })
                };
            }
            KeyCode::Char('[') => self.shift_offset(-OFFSET_STEP_MS),
            KeyCode::Char(']') => self.shift_offset(OFFSET_STEP_MS),
            KeyCode::Char('<') | KeyCode::Char(',') | KeyCode::PageUp => {
                self.goto_scene(self.scene.saturating_sub(1))
            }
            KeyCode::Char('>') | KeyCode::Char('.') | KeyCode::PageDown => {
                self.goto_scene(self.scene + 1)
            }
            KeyCode::Char(c @ '1'..='9') => self.toggle_card(c as usize - '0' as usize),
            KeyCode::Char(c @ ('d' | 'y' | 'm')) => {
                if !matches!(self.stage, Stage::Intensive | Stage::Relisten) {
                    self.message = "听写和标记在逐句精听/复听阶段使用".into();
                } else if c == 'd' {
                    self.input = Some(String::new());
                    self.message = "听写：输入听到的句子，Enter 提交，Tab 重播，Esc 取消".into();
                } else if c == 'y' {
                    self.set_mark(Mark::Clear);
                } else {
                    self.set_mark(Mark::Unclear);
                }
            }
            _ => {}
        }
    }

    fn on_input_key(&mut self, code: KeyCode) {
        let Some(input) = self.input.as_mut() else {
            return;
        };
        match code {
            KeyCode::Char(c) => input.push(c),
            KeyCode::Backspace => {
                input.pop();
            }
            KeyCode::Tab => self.replay(),
            KeyCode::Esc => {
                self.input = None;
                self.message = "已取消听写".into();
            }
            KeyCode::Enter => {
                let typed = self.input.take().unwrap_or_default();
                let diff = dictation::compare(&self.cues[self.cursor].one_line(), &typed);
                let (ok, total) = diff.score();
                let extra = diff.extra_words().len();
                self.message = if extra > 0 {
                    format!("听写 {ok}/{total} 词正确，多打 {extra} 词。y 听懂 · m 没听清")
                } else {
                    format!("听写 {ok}/{total} 词正确。y 听懂 · m 没听清")
                };
                self.dictation = Some(Dictation {
                    cue: self.cursor,
                    diff,
                });
            }
            _ => {}
        }
    }

    // ---- persistence ---------------------------------------------------

    fn save_position(&mut self) {
        let pos = self.player.position_ms();
        self.state.episode(&self.key).position_ms = pos;
        self.save();
    }

    fn save(&mut self) {
        if let Some(path) = &self.state_path
            && let Err(e) = self.state.save(path)
        {
            self.message = format!("保存进度失败：{e}");
        }
    }

    pub fn quit(&mut self) {
        self.pause();
        self.save_position();
        self.quit = true;
    }
}

pub fn shift(ms: u64, offset: i64) -> u64 {
    (ms as i64 + offset).max(0) as u64
}

pub fn fmt_offset(ms: i64) -> String {
    format!("{:+.2}s", ms as f64 / 1000.0)
}

pub fn mark_label(m: Mark) -> &'static str {
    match m {
        Mark::Clear => "听懂",
        Mark::Unclear => "没听清",
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::player::tests::manual;
    use std::cell::Cell;
    use std::rc::Rc;

    /// 60 synthetic sentences, 2 s each with 0.5 s gaps, and a long pause
    /// before sentence 25 so the default split gives two scenes.
    pub fn synthetic_cues() -> Vec<Cue> {
        let mut t = 1_000;
        (0..60)
            .map(|i| {
                if i == 24 {
                    t += 8_000;
                }
                let c = Cue {
                    start_ms: t,
                    end_ms: t + 2_000,
                    text: format!("This is synthetic line number {}.", i + 1),
                };
                t += 2_500;
                c
            })
            .collect()
    }

    pub fn synthetic_words() -> WordsFile {
        WordsFile::parse(
            r#"{"items":[
              {"cue":1,"text":"synthetic line","ipa":"sɪnˈθɛtɪk ˈlaɪn","meaning":"合成的台词"},
              {"cue":2,"text":"line number","meaning":"行号","note":"示例备注"},
              {"cue":2,"text":"this is","meaning":"这是"}
            ]}"#,
        )
        .unwrap()
    }

    pub fn app() -> (App, Rc<Cell<u64>>) {
        let (player, now) = manual(Some(200_000));
        let app = App::new(
            "ep.m4a".into(),
            "/media/ep.m4a".into(),
            synthetic_cues(),
            synthetic_words(),
            State::default(),
            None,
            Box::new(player),
        );
        (app, now)
    }

    fn keys(app: &mut App, s: &str) {
        for c in s.chars() {
            app.on_key(KeyCode::Char(c));
        }
    }

    #[test]
    fn default_scenes_and_preview() {
        let (app, _) = app();
        assert_eq!(app.scenes.len(), 2);
        assert_eq!((app.scenes[0].start, app.scenes[0].end), (0, 24));
        assert_eq!(app.stage, Stage::Preview);
        assert_eq!(app.preview_items().len(), 3);
    }

    #[test]
    fn full_scene_flow() {
        let (mut app, now) = app();
        app.on_key(KeyCode::Enter);
        assert_eq!(app.stage, Stage::Blind);
        assert!(app.player.is_playing());
        assert!(!app.text_visible());
        now.set(12_000);
        app.tick();
        assert_eq!(app.cursor, 4, "cursor follows playback");
        now.set(100_000);
        app.tick();
        assert!(app.message.contains("盲听结束"));

        app.on_key(KeyCode::Enter);
        assert_eq!(app.stage, Stage::Intensive);
        assert_eq!(app.cursor, 0);
        now.set(103_000);
        app.tick();
        assert!(!app.player.is_playing(), "auto-paused after the sentence");
        assert!(app.message.contains("播完了"));

        // Dictation reveals the sentence and diffs it.
        app.on_key(KeyCode::Char('d'));
        keys(&mut app, "this is synthetic line number one");
        app.on_key(KeyCode::Enter);
        let d = app.dictation.as_ref().unwrap();
        assert_eq!(d.diff.score(), (5, 6));
        assert!(app.text_visible());

        // Marking moves on to the next sentence.
        keys(&mut app, "m");
        assert_eq!(app.cursor, 1);
        assert_eq!(app.mark(0), Some(Mark::Unclear));
        assert!(app.dictation.is_none());
        for _ in 1..24 {
            keys(&mut app, "y");
        }
        assert!(app.message.contains("逐句精听完成"));
        assert_eq!(app.scene_marks(), (24, 1));

        app.on_key(KeyCode::Enter);
        assert_eq!(app.stage, Stage::Relisten);
        assert_eq!(app.relisten, [0]);
        assert_eq!(app.cursor, 0);
        assert!(app.player.is_playing());

        app.on_key(KeyCode::Enter);
        assert_eq!(app.stage, Stage::Final);
        app.on_key(KeyCode::Enter);
        assert_eq!((app.scene, app.stage), (1, Stage::Preview));
    }

    #[test]
    fn relisten_plays_queue_in_order() {
        let (mut app, now) = app();
        for i in [2, 5, 7] {
            app.state
                .episode("/media/ep.m4a")
                .marks
                .insert(i + 1, Mark::Unclear);
        }
        app.enter_stage(Stage::Relisten);
        assert_eq!(app.cursor, 2);
        let mut seen = vec![app.cursor];
        for _ in 0..3 {
            now.set(now.get() + 10_000);
            app.tick();
            seen.push(app.cursor);
        }
        assert_eq!(seen, [2, 5, 7, 7]);
        assert!(app.message.contains("回放完毕"));
    }

    #[test]
    fn favourites_toggle_and_record_source() {
        let (mut app, _) = app();
        app.enter_stage(Stage::Intensive);
        keys(&mut app, "j");
        assert_eq!(app.cursor, 1);
        keys(&mut app, "2");
        let card = &app.state.cards[0];
        assert_eq!(
            (card.file.as_str(), card.cue, card.expression.as_str()),
            ("/media/ep.m4a", 2, "this is")
        );
        assert_eq!(card.sentence, "This is synthetic line number 2.");
        keys(&mut app, "5");
        assert!(app.message.contains("只有 2 个"));
        keys(&mut app, "2");
        assert!(app.state.cards.is_empty());
    }

    #[test]
    fn offset_shifts_sentence_times() {
        let (mut app, _) = app();
        let before = app.cue_start(3);
        keys(&mut app, "]]]");
        assert_eq!(app.cue_start(3), before + 750);
        keys(&mut app, "[");
        assert_eq!(app.offset_ms(), 500);
        assert!(app.message.contains("+0.50s"));
    }

    #[test]
    fn space_continues_to_next_sentence() {
        let (mut app, now) = app();
        app.enter_stage(Stage::Intensive);
        now.set(5_000);
        app.tick();
        keys(&mut app, " ");
        assert_eq!(app.cursor, 1);
        assert!(app.player.is_playing());
        keys(&mut app, " ");
        assert!(!app.player.is_playing());
        keys(&mut app, " ");
        assert_eq!(app.cursor, 1, "resumes inside the sentence");
    }

    #[test]
    fn resumes_saved_position() {
        let (player, _) = manual(None);
        let mut state = State::default();
        state.episode("/media/ep.m4a").position_ms = 80_000;
        let app = App::new(
            "ep.m4a".into(),
            "/media/ep.m4a".into(),
            synthetic_cues(),
            WordsFile::default(),
            state,
            None,
            Box::new(player),
        );
        assert_eq!(app.scene, 1);
        assert_eq!(app.cursor, 24);
        assert_eq!(app.player.position_ms(), app.cue_start(24));
        let mut app = app;
        app.quit();
        assert_eq!(app.state.episodes["/media/ep.m4a"].position_ms, 69_000);
    }
}
