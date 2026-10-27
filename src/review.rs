//! Review mode: Leitner flashcards for saved expressions. Each card first
//! plays the original sentence, then flips to show expression, gloss and text.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use ratatui::Frame;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

use crate::app::{TAIL_MS, shift};
use crate::player::Player;
use crate::srt::Cue;
use crate::state::State;
use crate::ui::{ACCENT, DIM, help_popup, render_centered};

pub const REVIEW_HELP: &[(&str, &str)] = &[
    ("space", "播放 / 重播原句音频"),
    ("Enter", "翻面：看表达、释义和原句"),
    ("y", "记得（升一档，间隔 1/2/4/7/15 天）"),
    ("n", "忘了（回到第一档，明天再见）"),
    ("?", "帮助"),
    ("q", "保存并退出"),
];

/// Opens a player for an audio file; `duration` is a fallback length.
pub type Opener = Box<dyn FnMut(&Path, Option<u64>) -> Result<Box<dyn Player>, String>>;

pub struct Review {
    pub state: State,
    state_path: Option<PathBuf>,
    today: i64,
    /// Indices into `state.cards`, in review order.
    pub queue: Vec<usize>,
    pub pos: usize,
    pub flipped: bool,
    opener: Opener,
    /// The player for the audio file currently loaded.
    player: Option<(String, Box<dyn Player>)>,
    /// Parsed subtitles per audio file (`None`: not available).
    subtitles: HashMap<String, Option<Vec<Cue>>>,
    pub remembered: usize,
    pub forgot: usize,
    pub message: String,
    pub show_help: bool,
    pub quit: bool,
}

impl Review {
    /// Reviews cards due on `today` (all cards when `all` is set), oldest due first.
    pub fn new(
        state: State,
        state_path: Option<PathBuf>,
        today: i64,
        all: bool,
        opener: Opener,
    ) -> Review {
        let mut queue: Vec<usize> = (0..state.cards.len())
            .filter(|&i| all || state.cards[i].schedule.is_due(today))
            .collect();
        queue.sort_by_key(|&i| (state.cards[i].schedule.due, i));
        let mut r = Review {
            state,
            state_path,
            today,
            queue,
            pos: 0,
            flipped: false,
            opener,
            player: None,
            subtitles: HashMap::new(),
            remembered: 0,
            forgot: 0,
            message: String::new(),
            show_help: false,
            quit: false,
        };
        r.play_current();
        r
    }

    pub fn done(&self) -> bool {
        self.pos >= self.queue.len()
    }

    fn card_index(&self) -> Option<usize> {
        self.queue.get(self.pos).copied()
    }

    fn cues_for(&mut self, file: &str) -> Option<&Vec<Cue>> {
        self.subtitles
            .entry(file.to_string())
            .or_insert_with(|| {
                let srt = Path::new(file).with_extension("srt");
                std::fs::read_to_string(srt)
                    .ok()
                    .map(|t| crate::srt::parse(&t))
            })
            .as_ref()
    }

    /// Plays the current card's sentence from the original audio.
    pub fn play_current(&mut self) {
        let Some(idx) = self.card_index() else {
            return;
        };
        let card = self.state.cards[idx].clone();
        let offset = self
            .state
            .episodes
            .get(&card.file)
            .map_or(0, |e| e.offset_ms);
        let Some(cue) = self
            .cues_for(&card.file)
            .and_then(|c| c.get(card.cue.wrapping_sub(1)))
            .cloned()
        else {
            self.message = "找不到原句的字幕文件，无法播放".into();
            return;
        };
        if self.player.as_ref().is_none_or(|(f, _)| *f != card.file) {
            self.player = None;
            let duration = self
                .cues_for(&card.file)
                .and_then(|c| c.last())
                .map(|c| c.end_ms);
            match (self.opener)(Path::new(&card.file), duration) {
                Ok(p) => self.player = Some((card.file.clone(), p)),
                Err(e) => {
                    self.message = format!("无法播放原句：{e}");
                    return;
                }
            }
        }
        if let Some((_, p)) = self.player.as_mut() {
            p.play_range(
                shift(cue.start_ms, offset),
                Some(shift(cue.end_ms, offset) + TAIL_MS),
            );
            self.message = if p.audible() {
                "♪ 正在播放原句".into()
            } else {
                "静音模式：没有音频输出".into()
            };
        }
    }

    pub fn tick(&mut self) {
        if let Some((_, p)) = self.player.as_mut() {
            p.tick();
        }
    }

    fn answer(&mut self, remembered: bool) {
        let Some(idx) = self.card_index() else {
            return;
        };
        self.state.cards[idx]
            .schedule
            .answer(remembered, self.today);
        if remembered {
            self.remembered += 1;
        } else {
            self.forgot += 1;
        }
        self.save();
        self.pos += 1;
        self.flipped = false;
        self.message.clear();
        if let Some((_, p)) = self.player.as_mut() {
            p.pause();
        }
        self.play_current();
    }

    fn save(&mut self) {
        if let Some(path) = &self.state_path
            && let Err(e) = self.state.save(path)
        {
            self.message = format!("保存失败：{e}");
        }
    }

    pub fn on_key(&mut self, code: KeyCode) {
        if self.show_help {
            self.show_help = false;
            return;
        }
        match code {
            KeyCode::Char('q') | KeyCode::Esc => {
                if let Some((_, p)) = self.player.as_mut() {
                    p.pause();
                }
                self.quit = true;
            }
            KeyCode::Char('?') => self.show_help = true,
            _ if self.done() => {}
            KeyCode::Char(' ') => self.play_current(),
            KeyCode::Enter => self.flipped = true,
            KeyCode::Char(c @ ('y' | 'n')) => {
                if self.flipped {
                    self.answer(c == 'y');
                } else {
                    self.message = "先回想，再按 Enter 翻面，然后自评".into();
                }
            }
            _ => {}
        }
    }

    pub fn draw(&self, f: &mut Frame) {
        let [header, body, message, hints] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(3),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .areas(f.area());
        let title = if self.done() {
            " listen review · 完成".to_string()
        } else {
            format!(
                " listen review · 第 {}/{} 张",
                self.pos + 1,
                self.queue.len()
            )
        };
        f.render_widget(
            Paragraph::new(title).style(Style::new().fg(Color::Black).bg(ACCENT)),
            header,
        );
        let block = Block::bordered().border_style(Style::new().fg(DIM));
        let inner = block.inner(body);
        f.render_widget(block, body);
        let (pinned, lines) = match self.card_index() {
            None => (None, self.summary_lines()),
            Some(idx) => {
                let (meta, lines) = self.card_lines(idx);
                (Some(meta), lines)
            }
        };
        render_centered(f, inner, pinned, lines);
        f.render_widget(
            Paragraph::new(self.message.as_str()).fg(Color::Yellow),
            message,
        );
        let hint = if self.done() {
            " q 退出"
        } else if self.flipped {
            " y 记得 · n 忘了 · space 重播 · ? 帮助 · q 退出"
        } else {
            " space 重播原句 · Enter 翻面 · ? 帮助 · q 退出"
        };
        f.render_widget(
            Paragraph::new(hint).style(Style::new().fg(Color::Black).bg(DIM)),
            hints,
        );
        if self.show_help {
            help_popup(f, "复习按键", REVIEW_HELP);
        }
    }

    /// The card's source line (kept at the top) and its body.
    fn card_lines(&self, idx: usize) -> (Line<'static>, Vec<Line<'static>>) {
        let card = &self.state.cards[idx];
        let source = Path::new(&card.file)
            .file_name()
            .map_or(card.file.clone(), |n| n.to_string_lossy().into_owned());
        let meta = Line::styled(
            format!(
                "来源 {source} #{} · 第 {} 档",
                card.cue, card.schedule.level
            ),
            Style::new().fg(DIM),
        );
        if !self.flipped {
            let lines = vec![
                Line::from("听原句，回想其中的重点表达和它的意思。"),
                Line::raw(""),
                Line::styled("Enter 翻面", Style::new().fg(DIM)),
            ];
            return (meta, lines);
        }
        let mut lines = vec![
            Line::styled(
                card.expression.clone(),
                Style::new().fg(ACCENT).add_modifier(Modifier::BOLD),
            ),
        ];
        if let Some(ipa) = &card.ipa {
            lines.push(Line::styled(format!("/{ipa}/"), Style::new().fg(DIM)));
        }
        lines.push(Line::from(card.meaning.clone()));
        if let Some(note) = &card.note {
            lines.push(Line::styled(note.clone(), Style::new().fg(DIM)));
        }
        lines.push(Line::raw(""));
        lines.push(highlight(&card.sentence, &card.expression));
        lines.push(Line::raw(""));
        lines.push(Line::styled(
            "记得吗？y 记得 · n 忘了",
            Style::new().fg(DIM),
        ));
        (meta, lines)
    }

    fn summary_lines(&self) -> Vec<Line<'static>> {
        let mut lines = Vec::new();
        if self.queue.is_empty() {
            lines.push(Line::from("今天没有到期的卡片。"));
        } else {
            lines.push(Line::from(format!(
                "复习完成：记得 {}，忘了 {}。",
                self.remembered, self.forgot
            )));
        }
        let next = self
            .state
            .cards
            .iter()
            .map(|c| c.schedule.due)
            .filter(|&d| d > self.today)
            .min();
        lines.push(Line::raw(""));
        lines.push(Line::styled(
            match next {
                Some(d) if d == self.today + 1 => "下一批卡片明天到期。".to_string(),
                Some(d) => format!("下一批卡片 {} 天后到期。", d - self.today),
                None if self.state.cards.is_empty() => {
                    "还没有收藏：在 listen play 里按数字键收藏重点表达。".to_string()
                }
                None => String::new(),
            },
            Style::new().fg(DIM),
        ));
        lines
    }
}

/// The sentence with the first case-insensitive occurrence of `expr` highlighted.
fn highlight(sentence: &str, expr: &str) -> Line<'static> {
    let lower = sentence.to_ascii_lowercase();
    let found = lower
        .find(&expr.to_ascii_lowercase())
        .filter(|_| !expr.is_empty());
    match found {
        Some(at) if sentence.is_char_boundary(at + expr.len()) => Line::from(vec![
            Span::raw(sentence[..at].to_string()),
            Span::styled(
                sentence[at..at + expr.len()].to_string(),
                Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ),
            Span::raw(sentence[at + expr.len()..].to_string()),
        ]),
        _ => Line::from(sentence.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::leitner::Schedule;
    use crate::player::NullPlayer;
    use crate::state::Card;
    use crate::ui::tests::{assert_centered, panel_rows, render, text_of};
    use std::cell::RefCell;
    use std::rc::Rc;

    fn card(file: &str, cue: usize, expr: &str, due: i64) -> Card {
        Card {
            file: file.into(),
            cue,
            expression: expr.into(),
            meaning: "示例释义".into(),
            ipa: Some("ˈhæŋ ˈaʊt".into()),
            note: Some("示例备注".into()),
            sentence: format!("Well, {expr} is the point."),
            added: 0,
            schedule: Schedule { level: 2, due },
        }
    }

    fn review(cards: Vec<Card>, all: bool) -> (Review, Rc<RefCell<Vec<String>>>) {
        let opened = Rc::new(RefCell::new(Vec::new()));
        let log = opened.clone();
        let opener: Opener = Box::new(move |p: &Path, d| {
            log.borrow_mut().push(p.display().to_string());
            Ok(Box::new(NullPlayer::new(d)) as Box<dyn Player>)
        });
        let state = State {
            cards,
            ..State::default()
        };
        (Review::new(state, None, 100, all, opener), opened)
    }

    #[test]
    fn queue_has_due_cards_in_due_order() {
        let (r, _) = review(
            vec![
                card("/m/a.m4a", 1, "later", 101),
                card("/m/a.m4a", 2, "second", 100),
                card("/m/a.m4a", 3, "first", 90),
            ],
            false,
        );
        assert_eq!(r.queue, [2, 1]);
        let (r, _) = review(vec![card("/m/a.m4a", 1, "later", 101)], true);
        assert_eq!(r.queue, [0]);
    }

    #[test]
    fn flip_then_rate_updates_schedule() {
        let (mut r, _) = review(
            vec![
                card("/m/a.m4a", 1, "pick up", 100),
                card("/m/a.m4a", 2, "x", 99),
            ],
            false,
        );
        assert_eq!(r.queue, [1, 0]);
        r.on_key(KeyCode::Char('y'));
        assert_eq!(r.pos, 0, "must flip first");
        r.on_key(KeyCode::Enter);
        r.on_key(KeyCode::Char('n'));
        assert_eq!(r.state.cards[1].schedule, Schedule { level: 1, due: 101 });
        r.on_key(KeyCode::Enter);
        r.on_key(KeyCode::Char('y'));
        assert_eq!(r.state.cards[0].schedule, Schedule { level: 3, due: 104 });
        assert!(r.done());
        assert_eq!((r.remembered, r.forgot), (1, 1));
    }

    #[test]
    fn missing_subtitles_are_reported_not_fatal() {
        let (r, opened) = review(vec![card("/nonexistent/a.m4a", 1, "x", 0)], false);
        assert!(r.message.contains("找不到原句的字幕文件"));
        assert!(opened.borrow().is_empty());
    }

    #[test]
    fn plays_original_sentence_from_audio_file() {
        let dir = std::env::temp_dir().join(format!("ll-review-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let audio = dir.join("ep.m4a");
        std::fs::write(
            dir.join("ep.srt"),
            "1\n00:00:01,000 --> 00:00:02,000\nFirst.\n\n2\n00:00:03,000 --> 00:00:04,000\nSecond.\n",
        )
        .unwrap();
        let file = audio.display().to_string();
        let (mut r, opened) = review(vec![card(&file, 2, "a", 0), card(&file, 1, "b", 1)], false);
        assert_eq!(opened.borrow().as_slice(), [file.as_str()]);
        let p = &r.player.as_ref().unwrap().1;
        assert_eq!(p.position_ms(), 3_000);
        assert!(p.is_playing());
        r.on_key(KeyCode::Enter);
        r.on_key(KeyCode::Char('y'));
        assert_eq!(r.player.as_ref().unwrap().1.position_ms(), 1_000);
        assert_eq!(
            opened.borrow().len(),
            1,
            "player is reused for the same file"
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn renders_front_back_and_summary() {
        let (mut r, _) = review(vec![card("/m/ep01.m4a", 7, "hang out", 100)], false);
        let text = text_of(&render(80, 14, |f| r.draw(f)));
        assert!(text.contains("第 1/1 张"), "{text}");
        assert!(text.contains("来源 ep01.m4a #7 · 第 2 档"), "{text}");
        assert!(!text.contains("hang out"), "front hides the answer: {text}");
        assert!(!text.contains("示例释义"), "{text}");
        assert!(!text.contains("ˈhæŋ"), "front hides the pronunciation: {text}");

        r.on_key(KeyCode::Enter);
        let buf = render(80, 14, |f| r.draw(f));
        let text = text_of(&buf);
        assert!(text.contains("hang out"), "{text}");
        assert!(text.contains("示例释义"), "{text}");
        assert!(text.contains("/ˈhæŋ ˈaʊt/"), "{text}");
        assert!(text.contains("Well, hang out is the point."), "{text}");

        r.on_key(KeyCode::Char('y'));
        let text = text_of(&render(80, 14, |f| r.draw(f)));
        assert!(text.contains("复习完成：记得 1，忘了 0"), "{text}");
        assert!(text.contains("4 天后到期"), "{text}");
    }

    #[test]
    fn small_terminal_does_not_panic() {
        let (mut r, _) = review(vec![card("/m/a.m4a", 1, "x", 0)], false);
        render(12, 3, |f| r.draw(f));
        r.on_key(KeyCode::Enter);
        r.show_help = true;
        render(12, 3, |f| r.draw(f));
    }

    #[test]
    fn card_content_is_vertically_centred() {
        let (mut r, _) = review(vec![card("/m/ep01.m4a", 7, "hang out", 100)], false);
        let text = text_of(&render(80, 24, |f| r.draw(f)));
        assert!(
            panel_rows(&text)[0].starts_with("来源 ep01.m4a #7"),
            "{text}"
        );
        assert_centered(&text, 1);

        r.on_key(KeyCode::Enter);
        let text = text_of(&render(80, 24, |f| r.draw(f)));
        assert!(
            panel_rows(&text)[0].starts_with("来源 ep01.m4a #7"),
            "{text}"
        );
        assert!(text.contains("Well, hang out is the point."), "{text}");
        assert_centered(&text, 1);

        r.on_key(KeyCode::Char('y'));
        let text = text_of(&render(80, 24, |f| r.draw(f)));
        assert!(text.contains("复习完成"), "{text}");
        assert_centered(&text, 0);
    }

    #[test]
    fn renders_empty_state() {
        let (r, _) = review(vec![], false);
        let text = text_of(&render(80, 10, |f| r.draw(f)));
        assert!(text.contains("今天没有到期的卡片"), "{text}");
        assert!(text.contains("还没有收藏"), "{text}");
    }

    #[test]
    fn highlight_is_case_insensitive() {
        let l = highlight("Come ON, guys.", "come on");
        assert_eq!(l.spans[1].content, "Come ON");
        assert_eq!(highlight("abc", "zzz").spans.len(), 1);
    }
}
