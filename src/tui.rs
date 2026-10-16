//! Terminal setup and the event loop shared by play and review modes.

use std::io;
use std::time::Duration;

use ratatui::Frame;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};

use crate::app::App;
use crate::review::Review;

pub trait Screen {
    fn draw(&self, f: &mut Frame);
    fn key(&mut self, code: KeyCode);
    fn tick(&mut self);
    /// Ctrl-C: leave the same way as `q`.
    fn interrupt(&mut self);
    fn finished(&self) -> bool;
}

impl Screen for App {
    fn draw(&self, f: &mut Frame) {
        crate::ui::draw(f, self);
    }
    fn key(&mut self, code: KeyCode) {
        self.on_key(code);
    }
    fn tick(&mut self) {
        App::tick(self);
    }
    fn interrupt(&mut self) {
        self.quit();
    }
    fn finished(&self) -> bool {
        self.quit
    }
}

impl Screen for Review {
    fn draw(&self, f: &mut Frame) {
        Review::draw(self, f);
    }
    fn key(&mut self, code: KeyCode) {
        self.on_key(code);
    }
    fn tick(&mut self) {
        Review::tick(self);
    }
    fn interrupt(&mut self) {
        self.on_key(KeyCode::Char('q'));
    }
    fn finished(&self) -> bool {
        self.quit
    }
}

pub fn run(screen: &mut impl Screen) -> io::Result<()> {
    let mut terminal = ratatui::try_init()?;
    let result = event_loop(&mut terminal, screen);
    ratatui::restore();
    result
}

fn event_loop(terminal: &mut ratatui::DefaultTerminal, screen: &mut impl Screen) -> io::Result<()> {
    while !screen.finished() {
        terminal.draw(|f| screen.draw(f))?;
        if event::poll(Duration::from_millis(50))?
            && let Event::Key(key) = event::read()?
            && key.kind != KeyEventKind::Release
        {
            if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                screen.interrupt();
            } else {
                screen.key(key.code);
            }
        }
        screen.tick();
    }
    Ok(())
}
