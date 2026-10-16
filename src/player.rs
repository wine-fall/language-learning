//! Audio playback behind a small trait, so everything else can be built and
//! tested without an audio device (`--no-default-features`).

use std::path::Path;
use std::time::Instant;

pub trait Player {
    /// Plays from `start_ms` and pauses by itself once `end_ms` is reached
    /// (`None` plays to the end of the file).
    fn play_range(&mut self, start_ms: u64, end_ms: Option<u64>);
    fn pause(&mut self);
    /// Continues from the current position, still stopping at the current
    /// range end unless that end has already been reached.
    fn resume(&mut self);
    /// Moves to `ms`, keeping the play/pause state and clearing the range end.
    fn seek(&mut self, ms: u64);
    fn position_ms(&self) -> u64;
    fn is_playing(&self) -> bool;
    fn duration_ms(&self) -> Option<u64>;
    /// Called regularly by the UI loop to enforce the range end.
    fn tick(&mut self) {}
    /// Whether real audio is being produced.
    fn audible(&self) -> bool {
        true
    }
}

/// Opens the audio file with the best available backend.
#[cfg(feature = "audio")]
pub fn open(path: &Path, fallback_duration_ms: Option<u64>) -> Result<Box<dyn Player>, String> {
    let _ = fallback_duration_ms;
    crate::audio::RodioPlayer::open(path).map(|p| Box::new(p) as Box<dyn Player>)
}

/// Built without the `audio` feature: a silent, clock-driven player.
#[cfg(not(feature = "audio"))]
pub fn open(path: &Path, fallback_duration_ms: Option<u64>) -> Result<Box<dyn Player>, String> {
    let _ = path;
    Ok(Box::new(NullPlayer::new(fallback_duration_ms)))
}

/// A silent player that only keeps time. Used when the `audio` feature is
/// off and in tests.
pub struct NullPlayer {
    clock: Box<dyn Fn() -> u64>,
    /// Position when playback last started or was paused.
    base_ms: u64,
    /// Clock reading when playback started; `None` while paused.
    started: Option<u64>,
    end_ms: Option<u64>,
    duration_ms: Option<u64>,
}

impl NullPlayer {
    pub fn new(duration_ms: Option<u64>) -> Self {
        let origin = Instant::now();
        Self::with_clock(
            duration_ms,
            Box::new(move || origin.elapsed().as_millis() as u64),
        )
    }

    /// Uses `clock` (milliseconds, monotonic) instead of the wall clock.
    pub fn with_clock(duration_ms: Option<u64>, clock: Box<dyn Fn() -> u64>) -> Self {
        NullPlayer {
            clock,
            base_ms: 0,
            started: None,
            end_ms: None,
            duration_ms,
        }
    }

    fn limit(&self) -> Option<u64> {
        match (self.end_ms, self.duration_ms) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }

    fn raw_position(&self) -> u64 {
        let pos = match self.started {
            Some(t) => self.base_ms + (self.clock)().saturating_sub(t),
            None => self.base_ms,
        };
        match self.limit() {
            Some(l) if self.started.is_some() => pos.min(l.max(self.base_ms)),
            _ => pos,
        }
    }
}

impl Player for NullPlayer {
    fn play_range(&mut self, start_ms: u64, end_ms: Option<u64>) {
        self.base_ms = start_ms;
        self.end_ms = end_ms;
        self.started = Some((self.clock)());
    }

    fn pause(&mut self) {
        self.base_ms = self.raw_position();
        self.started = None;
    }

    fn resume(&mut self) {
        if self.started.is_some() {
            return;
        }
        if self.end_ms.is_some_and(|e| self.base_ms >= e) {
            self.end_ms = None;
        }
        self.started = Some((self.clock)());
    }

    fn seek(&mut self, ms: u64) {
        self.base_ms = ms;
        self.end_ms = None;
        if self.started.is_some() {
            self.started = Some((self.clock)());
        }
    }

    fn position_ms(&self) -> u64 {
        self.raw_position()
    }

    fn is_playing(&self) -> bool {
        self.started.is_some() && self.limit().is_none_or(|l| self.raw_position() < l)
    }

    fn duration_ms(&self) -> Option<u64> {
        self.duration_ms
    }

    fn tick(&mut self) {
        if self.started.is_some() && !self.is_playing() {
            self.pause();
        }
    }

    fn audible(&self) -> bool {
        false
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;

    /// A NullPlayer driven by a manual clock.
    pub fn manual(duration: Option<u64>) -> (NullPlayer, Rc<Cell<u64>>) {
        let now = Rc::new(Cell::new(0));
        let c = now.clone();
        (
            NullPlayer::with_clock(duration, Box::new(move || c.get())),
            now,
        )
    }

    #[test]
    fn plays_and_stops_at_range_end() {
        let (mut p, now) = manual(Some(60_000));
        assert!(!p.is_playing());
        p.play_range(1_000, Some(2_500));
        assert!(p.is_playing());
        now.set(1_000);
        assert_eq!(p.position_ms(), 2_000);
        now.set(5_000);
        assert_eq!(p.position_ms(), 2_500);
        assert!(!p.is_playing());
        p.tick();
        assert_eq!(p.position_ms(), 2_500);
        // Resuming after the end continues past it.
        p.resume();
        now.set(5_500);
        assert_eq!(p.position_ms(), 3_000);
        assert!(p.is_playing());
    }

    #[test]
    fn pause_resume_and_seek() {
        let (mut p, now) = manual(None);
        p.play_range(0, Some(10_000));
        now.set(3_000);
        p.pause();
        now.set(8_000);
        assert_eq!(p.position_ms(), 3_000);
        assert!(!p.is_playing());
        p.resume();
        now.set(20_000);
        assert_eq!(p.position_ms(), 10_000, "range end still applies");
        p.seek(500);
        assert_eq!(p.position_ms(), 500);
        assert!(!p.audible());
    }

    #[test]
    fn stops_at_duration() {
        let (mut p, now) = manual(Some(4_000));
        p.play_range(3_000, None);
        now.set(9_000);
        assert_eq!(p.position_ms(), 4_000);
        assert!(!p.is_playing());
    }
}
