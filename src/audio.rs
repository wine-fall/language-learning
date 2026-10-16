//! Real playback through rodio (feature `audio`).
//!
//! Kept deliberately small: one decoder appended to one rodio `Player`, with
//! seeking via `try_seek` and the range end enforced from `tick`.

use std::fs::File;
use std::path::{Path, PathBuf};
use std::time::Duration;

use rodio::Source;

pub struct RodioPlayer {
    path: PathBuf,
    // Field order matters: the player must drop before the device sink.
    player: rodio::Player,
    _sink: rodio::MixerDeviceSink,
    end_ms: Option<u64>,
    duration_ms: Option<u64>,
}

impl RodioPlayer {
    pub fn open(path: &Path) -> Result<Self, String> {
        let mut sink = rodio::DeviceSinkBuilder::open_default_sink()
            .map_err(|e| format!("cannot open audio output: {e}"))?;
        sink.log_on_drop(false);
        let player = rodio::Player::connect_new(sink.mixer());
        player.pause();
        let mut this = RodioPlayer {
            path: path.to_path_buf(),
            player,
            _sink: sink,
            end_ms: None,
            duration_ms: None,
        };
        this.load()?;
        Ok(this)
    }

    /// (Re)loads the decoder; needed again once playback ran off the end,
    /// because rodio drops a finished source.
    fn load(&mut self) -> Result<(), String> {
        let file = File::open(&self.path)
            .map_err(|e| format!("cannot open {}: {e}", self.path.display()))?;
        let decoder = rodio::Decoder::try_from(file)
            .map_err(|e| format!("cannot decode {}: {e}", self.path.display()))?;
        if self.duration_ms.is_none() {
            self.duration_ms = decoder.total_duration().map(|d| d.as_millis() as u64);
        }
        self.player.append(decoder);
        Ok(())
    }

    fn seek_to(&mut self, ms: u64) {
        if self.player.empty() && self.load().is_err() {
            return;
        }
        // Seeking can fail on unusual files; playback then continues from
        // wherever it is, which is the least surprising fallback.
        let _ = self.player.try_seek(Duration::from_millis(ms));
    }
}

impl crate::player::Player for RodioPlayer {
    fn play_range(&mut self, start_ms: u64, end_ms: Option<u64>) {
        self.seek_to(start_ms);
        self.end_ms = end_ms;
        self.player.play();
    }

    fn pause(&mut self) {
        self.player.pause();
    }

    fn resume(&mut self) {
        if self.end_ms.is_some_and(|e| self.position_ms() >= e) {
            self.end_ms = None;
        }
        if self.player.empty() {
            let pos = self.position_ms();
            self.seek_to(pos);
        }
        self.player.play();
    }

    fn seek(&mut self, ms: u64) {
        self.end_ms = None;
        self.seek_to(ms);
    }

    fn position_ms(&self) -> u64 {
        self.player.get_pos().as_millis() as u64
    }

    fn is_playing(&self) -> bool {
        !self.player.is_paused() && !self.player.empty()
    }

    fn duration_ms(&self) -> Option<u64> {
        self.duration_ms
    }

    fn tick(&mut self) {
        if let Some(end) = self.end_ms
            && !self.player.is_paused()
            && self.position_ms() >= end
        {
            self.player.pause();
        }
    }
}
