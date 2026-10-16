//! Rendering for play mode, plus small helpers shared with review mode.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Gauge, Paragraph, Wrap};

use crate::app::{App, Stage, fmt_offset, mark_label};
use crate::srt::fmt_time;
use crate::state::Mark;

pub const ACCENT: Color = Color::Cyan;
pub const OK: Color = Color::Green;
pub const BAD: Color = Color::Red;
pub const DIM: Color = Color::DarkGray;

pub const PLAY_HELP: &[(&str, &str)] = &[
    (
        "Enter",
        "进入下一阶段（预习→盲听→逐句精听→复听→无字幕连播→下一场景）",
    ),
    ("space", "暂停 / 继续；精听时一句播完后按它播下一句"),
    ("j / k", "下一句 / 上一句（也可用方向键）"),
    ("r", "重播当前句（盲听/连播时从场景开头重放）"),
    ("p", "逐句自动暂停 开/关"),
    ("c", "字幕 关 / 英文全文"),
    ("d", "听写当前句（Enter 提交，Tab 重播，Esc 取消）"),
    ("y / m", "标记 听懂 / 没听清，并进入下一句"),
    (
        "1-9",
        "收藏当前句（预习页：列表中）第 N 个重点表达，再按取消",
    ),
    ("[ / ]", "字幕时间轴 -0.25s / +0.25s（按文件保存）"),
    ("< / >", "上一场景 / 下一场景"),
    ("?", "帮助"),
    ("q", "保存并退出"),
];

pub fn draw(f: &mut Frame, app: &App) {
    let [header, tabs, body, status, message, hints] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(3),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(f.area());

    let scene = app.current_scene();
    let mut title = format!(
        " listen · {} · 场景 {}/{}",
        app.title,
        app.scene + 1,
        app.scenes.len()
    );
    if let Some(t) = &scene.title {
        title.push_str(&format!("「{t}」"));
    }
    f.render_widget(
        Paragraph::new(title).style(Style::new().fg(Color::Black).bg(ACCENT)),
        header,
    );
    f.render_widget(stage_tabs(app.stage), tabs);

    let block = Block::bordered()
        .title(format!(" {} ", app.stage.label()))
        .border_style(Style::new().fg(DIM));
    let inner = block.inner(body);
    f.render_widget(block, body);
    match app.stage {
        Stage::Preview => draw_preview(f, app, inner),
        Stage::Blind | Stage::Final => draw_continuous(f, app, inner),
        Stage::Intensive => draw_sentence(f, app, inner),
        Stage::Relisten => draw_relisten(f, app, inner),
    }

    f.render_widget(status_line(app), status);
    f.render_widget(
        Paragraph::new(app.message.as_str()).fg(Color::Yellow),
        message,
    );
    f.render_widget(hint_line(app), hints);

    if app.show_help {
        help_popup(f, "按键", PLAY_HELP);
    }
}

fn stage_tabs(current: Stage) -> Paragraph<'static> {
    let mut spans = vec![Span::raw(" ")];
    for (n, stage) in Stage::ALL.iter().enumerate() {
        if n > 0 {
            spans.push(Span::styled(" › ", Style::new().fg(DIM)));
        }
        let label = format!("{} {}", ["①", "②", "③", "④", "④"][n], stage.label());
        spans.push(if *stage == current {
            Span::styled(label, Style::new().fg(ACCENT).add_modifier(Modifier::BOLD))
        } else {
            Span::styled(label, Style::new().fg(DIM))
        });
    }
    Paragraph::new(Line::from(spans))
}

fn draw_preview(f: &mut Frame, app: &App, area: Rect) {
    let items = app.preview_items();
    let scene = app.current_scene();
    let mut lines = vec![
        Line::from(format!(
            "本场景 {} 句（#{}–#{}）。先看看这些表达，再去听：",
            scene.len(),
            scene.start + 1,
            scene.end
        )),
        Line::raw(""),
    ];
    if items.is_empty() {
        lines.push(Line::styled(
            "（这一场没有重点表达：没有 words.json，或其中没有本场景的条目）",
            Style::new().fg(DIM),
        ));
    }
    for (n, item) in items.iter().enumerate() {
        let mut spans = vec![
            Span::styled(format!(" {} ", n + 1), Style::new().fg(DIM)),
            star(app.is_saved(item)),
            Span::styled(item.text.clone(), Style::new().add_modifier(Modifier::BOLD)),
            Span::raw("  "),
            Span::raw(item.meaning.clone()),
        ];
        if let Some(note) = &item.note {
            spans.push(Span::styled(format!("  ({note})"), Style::new().fg(DIM)));
        }
        lines.push(Line::from(spans));
    }
    lines.push(Line::raw(""));
    lines.push(Line::styled(
        "Enter 开始盲听 · 数字键收藏",
        Style::new().fg(DIM),
    ));
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), area);
}

fn draw_continuous(f: &mut Frame, app: &App, area: Rect) {
    let [top, gauge, rest] = Layout::vertical([
        Constraint::Length(2),
        Constraint::Length(1),
        Constraint::Min(0),
    ])
    .areas(area);
    let what = if app.stage == Stage::Blind {
        "盲听：不看字幕，把整场连续听一遍，抓大意。"
    } else {
        "最后一遍：无字幕连续播放整场，检验这一场是否都能听懂。"
    };
    f.render_widget(Paragraph::new(what), top);

    let (start, end) = (app.scene_start_ms(), app.scene_end_ms());
    let pos = app.player.position_ms().clamp(start, end);
    let ratio = (pos - start) as f64 / (end - start).max(1) as f64;
    let scene = app.current_scene();
    f.render_widget(
        Gauge::default()
            .gauge_style(Style::new().fg(ACCENT).bg(Color::Reset))
            .ratio(ratio.clamp(0.0, 1.0))
            .label(format!(
                "第 {}/{} 句 · {} / {}",
                app.cursor - scene.start + 1,
                scene.len(),
                fmt_time(pos - start),
                fmt_time(end - start)
            )),
        gauge,
    );
    f.render_widget(Paragraph::new("（字幕已隐藏）").fg(DIM), rest);
}

fn draw_sentence(f: &mut Frame, app: &App, area: Rect) {
    let scene = app.current_scene();
    let (marked, unclear) = app.scene_marks();
    let mut lines = vec![
        Line::from(vec![
            Span::styled(
                format!("第 {}/{} 句", app.cursor - scene.start + 1, scene.len()),
                Style::new().add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!("  #{}", app.cursor + 1), Style::new().fg(DIM)),
            Span::raw("  "),
            mark_span(app.mark(app.cursor)),
            Span::styled(
                format!("    本场已标记 {marked}/{} · 没听清 {unclear}", scene.len()),
                Style::new().fg(DIM),
            ),
        ]),
        Line::raw(""),
    ];
    lines.extend(sentence_lines(app, app.cursor));
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), area);
}

fn draw_relisten(f: &mut Frame, app: &App, area: Rect) {
    let mut lines = vec![
        Line::from("复听：只回放标记为“没听清”的句子。space 依次播放，j/k 选择，y 改标听懂。"),
        Line::raw(""),
    ];
    if app.relisten.is_empty() {
        lines.push(Line::styled("（没有需要复听的句子）", Style::new().fg(DIM)));
    }
    for &i in &app.relisten {
        let selected = i == app.cursor;
        let text = if app.subtitles {
            app.cues[i].one_line()
        } else {
            "· · ·".to_string()
        };
        let style = if selected {
            Style::new().fg(ACCENT).add_modifier(Modifier::BOLD)
        } else {
            Style::new()
        };
        lines.push(Line::from(vec![
            Span::styled(if selected { " ▶ " } else { "   " }, style),
            Span::styled(format!("#{:<4} ", i + 1), Style::new().fg(DIM)),
            mark_span(app.mark(i)),
            Span::raw(" "),
            Span::styled(text, style),
        ]));
    }
    if app.relisten.contains(&app.cursor) && !app.subtitles {
        lines.push(Line::raw(""));
        lines.extend(sentence_lines(app, app.cursor));
    }
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), area);
}

/// The current sentence: subtitle or dictation result, input box, expressions.
fn sentence_lines(app: &App, i: usize) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let dictation = app.dictation.as_ref().filter(|d| d.cue == i);
    if let Some(d) = dictation {
        lines.push(diff_line(&d.diff.reference, false));
        lines.push(Line::raw(""));
        let mut typed = vec![Span::styled("你的听写：", Style::new().fg(DIM))];
        typed.extend(diff_line(&d.diff.typed, true).spans);
        lines.push(Line::from(typed));
        let (ok, total) = d.diff.score();
        lines.push(Line::styled(
            format!("{ok}/{total} 词正确（绿色=听对，红色=漏听或听错）"),
            Style::new().fg(DIM),
        ));
    } else if app.text_visible() {
        for l in app.cues[i].text.split('\n') {
            lines.push(Line::from(l.to_string()).add_modifier(Modifier::BOLD));
        }
    } else {
        lines.push(Line::styled(
            "（字幕已关：先听，按 d 听写，或按 c 打开字幕）",
            Style::new().fg(DIM),
        ));
    }
    if let Some(input) = &app.input {
        lines.push(Line::raw(""));
        lines.push(Line::from(vec![
            Span::styled("听写> ", Style::new().fg(ACCENT)),
            Span::raw(input.clone()),
            Span::styled("▏", Style::new().fg(ACCENT)),
        ]));
    }
    let items = app.items_for(i);
    if !items.is_empty() {
        lines.push(Line::raw(""));
        if app.text_visible() {
            let mut spans = vec![Span::styled("重点表达：", Style::new().fg(DIM))];
            for (n, item) in items.iter().enumerate() {
                spans.push(Span::styled(format!("{} ", n + 1), Style::new().fg(DIM)));
                spans.push(star(app.is_saved(item)));
                spans.push(Span::raw(format!("{}   ", item.text)));
            }
            lines.push(Line::from(spans));
        } else {
            lines.push(Line::styled(
                format!(
                    "本句有 {} 个重点表达（看过原句后可按 1-{} 收藏）",
                    items.len(),
                    items.len()
                ),
                Style::new().fg(DIM),
            ));
        }
    }
    lines
}

/// Words coloured by whether they matched. Typed words that were wrong are
/// crossed out; reference words that were missed are underlined.
pub fn diff_line(words: &[(String, bool)], typed: bool) -> Line<'static> {
    let mut spans = Vec::new();
    for (n, (w, ok)) in words.iter().enumerate() {
        if n > 0 {
            spans.push(Span::raw(" "));
        }
        let style = if *ok {
            Style::new().fg(OK)
        } else if typed {
            Style::new().fg(BAD).add_modifier(Modifier::CROSSED_OUT)
        } else {
            Style::new()
                .fg(BAD)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
        };
        spans.push(Span::styled(w.clone(), style));
    }
    if words.is_empty() {
        spans.push(Span::styled("（空）", Style::new().fg(DIM)));
    }
    Line::from(spans)
}

fn star(saved: bool) -> Span<'static> {
    if saved {
        Span::styled("★ ", Style::new().fg(Color::Yellow))
    } else {
        Span::styled("☆ ", Style::new().fg(DIM))
    }
}

fn mark_span(mark: Option<Mark>) -> Span<'static> {
    match mark {
        Some(Mark::Clear) => Span::styled(
            format!("[{}]", mark_label(Mark::Clear)),
            Style::new().fg(OK),
        ),
        Some(Mark::Unclear) => Span::styled(
            format!("[{}]", mark_label(Mark::Unclear)),
            Style::new().fg(BAD),
        ),
        None => Span::styled("[未标记]", Style::new().fg(DIM)),
    }
}

fn status_line(app: &App) -> Paragraph<'static> {
    let pos = app.player.position_ms();
    let total = app
        .player
        .duration_ms()
        .or_else(|| app.cues.last().map(|c| c.end_ms))
        .unwrap_or(0);
    let on_off = |b: bool| if b { "开" } else { "关" };
    let subs = if app.stage.hides_subtitles() {
        "隐藏"
    } else if app.subtitles {
        "英文"
    } else {
        "关"
    };
    let mut text = format!(
        " {} {} / {} · 字幕 {} · 逐句暂停 {} · 平移 {}",
        if app.player.is_playing() {
            "▶"
        } else {
            "⏸"
        },
        fmt_time(pos),
        fmt_time(total),
        subs,
        on_off(app.auto_pause),
        fmt_offset(app.offset_ms()),
    );
    if !app.player.audible() {
        text.push_str(" · 静音模式");
    }
    Paragraph::new(text).fg(DIM)
}

fn hint_line(app: &App) -> Paragraph<'static> {
    let hint = if app.input.is_some() {
        "输入听到的内容 · Enter 提交 · Tab 重播 · Esc 取消"
    } else {
        match app.stage {
            Stage::Preview => "Enter 开始盲听 · 1-9 收藏 · < > 换场景 · ? 帮助 · q 退出",
            Stage::Blind | Stage::Final => {
                "space 暂停/继续 · r 从头 · Enter 下一阶段 · ? 帮助 · q 退出"
            }
            Stage::Intensive => {
                "space 继续 · r 重播 · d 听写 · y 听懂 · m 没听清 · c 字幕 · j/k 换句 · Enter 复听 · ?"
            }
            Stage::Relisten => {
                "space 依次回放 · j/k 选句 · r 重播 · d 听写 · y/m 改标 · Enter 连播 · ?"
            }
        }
    };
    Paragraph::new(format!(" {hint}")).style(Style::new().fg(Color::Black).bg(DIM))
}

pub fn help_popup(f: &mut Frame, title: &str, keys: &[(&str, &str)]) {
    let width = 72.min(f.area().width);
    let height = (keys.len() as u16 + 4).min(f.area().height);
    let area = Rect {
        x: f.area().width.saturating_sub(width) / 2,
        y: f.area().height.saturating_sub(height) / 2,
        width,
        height,
    };
    let mut lines: Vec<Line> = keys
        .iter()
        .map(|(k, v)| {
            Line::from(vec![
                Span::styled(
                    format!(" {k:<7} "),
                    Style::new().fg(ACCENT).add_modifier(Modifier::BOLD),
                ),
                Span::raw(v.to_string()),
            ])
        })
        .collect();
    lines.push(Line::raw(""));
    lines.push(Line::styled(" 按任意键关闭", Style::new().fg(DIM)));
    f.render_widget(Clear, area);
    f.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: false }).block(
            Block::bordered()
                .title(format!(" {title} "))
                .border_style(Style::new().fg(ACCENT)),
        ),
        area,
    );
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::app::tests::app;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;
    use ratatui::crossterm::event::KeyCode;

    /// Renders `draw` into a test terminal and returns the buffer.
    pub fn render(width: u16, height: u16, draw: impl FnOnce(&mut Frame)) -> Buffer {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(draw).unwrap();
        terminal.backend().buffer().clone()
    }

    /// Buffer contents as plain text lines (wide characters de-duplicated).
    pub fn text_of(buf: &Buffer) -> String {
        let mut out = String::new();
        for y in 0..buf.area.height {
            let mut skip = 0;
            for x in 0..buf.area.width {
                if skip > 0 {
                    skip -= 1;
                    continue;
                }
                let sym = buf[(x, y)].symbol();
                out.push_str(sym);
                skip = Span::raw(sym).width().saturating_sub(1);
            }
            out.push('\n');
        }
        out
    }

    /// Style of the first cell where `needle` (ASCII) starts.
    fn style_of(buf: &Buffer, needle: &str) -> Style {
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                let mut s = String::new();
                for dx in 0..needle.len() as u16 {
                    if x + dx < buf.area.width {
                        s.push_str(buf[(x + dx, y)].symbol());
                    }
                }
                if s == needle {
                    return buf[(x, y)].style();
                }
            }
        }
        panic!("{needle:?} not found in\n{}", text_of(buf));
    }

    #[test]
    fn renders_preview_page() {
        let (app, _) = app();
        let buf = render(90, 20, |f| draw(f, &app));
        let text = text_of(&buf);
        assert!(text.contains("listen · ep.m4a · 场景 1/2"), "{text}");
        assert!(text.contains("① 预习"), "{text}");
        assert!(text.contains("synthetic line"), "{text}");
        assert!(text.contains("合成的台词"), "{text}");
        assert!(text.contains("示例备注"), "{text}");
        assert!(text.contains("静音模式"), "{text}");
    }

    #[test]
    fn renders_blind_stage_without_text() {
        let (mut app, _) = app();
        app.on_key(KeyCode::Enter);
        let text = text_of(&render(90, 20, |f| draw(f, &app)));
        assert!(text.contains("盲听"), "{text}");
        assert!(text.contains("第 1/24 句"), "{text}");
        assert!(!text.contains("synthetic line number"), "{text}");
    }

    #[test]
    fn renders_sentence_subtitles_and_no_chinese() {
        let (mut app, _) = app();
        app.enter_stage(Stage::Intensive);
        let text = text_of(&render(100, 20, |f| draw(f, &app)));
        assert!(text.contains("字幕已关"), "{text}");
        assert!(!text.contains("This is synthetic"), "{text}");
        assert!(text.contains("本句有 1 个重点表达"), "{text}");

        app.on_key(KeyCode::Char('c'));
        let text = text_of(&render(100, 20, |f| draw(f, &app)));
        assert!(text.contains("This is synthetic line number 1."), "{text}");
        assert!(text.contains("1 ☆ synthetic line"), "{text}");
        assert!(
            !text.contains("合成的台词"),
            "Chinese gloss only on preview/cards"
        );
    }

    #[test]
    fn renders_dictation_diff_colours() {
        let (mut app, _) = app();
        app.enter_stage(Stage::Intensive);
        app.on_key(KeyCode::Char('d'));
        for c in "this is synthetic lime number 1".chars() {
            app.on_key(KeyCode::Char(c));
        }
        let text = text_of(&render(100, 20, |f| draw(f, &app)));
        assert!(
            text.contains("听写> this is synthetic lime number 1"),
            "{text}"
        );
        app.on_key(KeyCode::Enter);
        let buf = render(100, 20, |f| draw(f, &app));
        let text = text_of(&buf);
        assert!(
            text.contains("你的听写：this is synthetic lime number 1"),
            "{text}"
        );
        assert!(text.contains("5/6 词正确"), "{text}");
        assert_eq!(style_of(&buf, "line").fg, Some(BAD));
        assert!(
            style_of(&buf, "lime")
                .add_modifier
                .contains(Modifier::CROSSED_OUT)
        );
        assert_eq!(style_of(&buf, "synthetic").fg, Some(OK));
    }

    #[test]
    fn renders_help_overlay() {
        let (mut app, _) = app();
        app.on_key(KeyCode::Char('?'));
        let text = text_of(&render(90, 24, |f| draw(f, &app)));
        assert!(text.contains("逐句自动暂停"), "{text}");
        app.on_key(KeyCode::Char('x'));
        assert!(!app.show_help);
    }

    #[test]
    fn small_terminal_does_not_panic() {
        let (mut app, _) = app();
        for stage in Stage::ALL {
            app.enter_stage(stage);
            render(20, 6, |f| draw(f, &app));
        }
        app.show_help = true;
        render(10, 3, |f| draw(f, &app));
    }
}
