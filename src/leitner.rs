//! Leitner-box scheduling for review cards.
//!
//! Five boxes with review intervals of 1, 2, 4, 7 and 15 days. Remembering a
//! card moves it up one box; forgetting it sends it back to the first box.

use serde::{Deserialize, Serialize};

pub const INTERVALS: [i64; 5] = [1, 2, 4, 7, 15];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Schedule {
    /// 1-based box number, `1..=INTERVALS.len()`.
    #[serde(rename = "box")]
    pub level: u8,
    /// Day number (see [`today`]) on or after which the card is due.
    pub due: i64,
}

impl Schedule {
    /// A new card goes into the first box and is due the next day.
    pub fn new(today: i64) -> Self {
        Schedule {
            level: 1,
            due: today + INTERVALS[0],
        }
    }

    pub fn is_due(&self, today: i64) -> bool {
        self.due <= today
    }

    pub fn answer(&mut self, remembered: bool, today: i64) {
        let max = INTERVALS.len() as u8;
        self.level = if remembered {
            (self.level.clamp(1, max) + 1).min(max)
        } else {
            1
        };
        self.due = today + INTERVALS[self.level as usize - 1];
    }
}

/// Days since 1970-01-01 in UTC.
pub fn today() -> i64 {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    (secs / 86_400) as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_card_is_due_tomorrow() {
        let s = Schedule::new(100);
        assert_eq!(s, Schedule { level: 1, due: 101 });
        assert!(!s.is_due(100));
        assert!(s.is_due(101));
    }

    #[test]
    fn remembering_climbs_boxes_and_caps() {
        let mut s = Schedule::new(0);
        let mut seen = vec![];
        for day in [1, 3, 7, 14, 29, 44] {
            s.answer(true, day);
            seen.push((s.level, s.due - day));
        }
        assert_eq!(seen, [(2, 2), (3, 4), (4, 7), (5, 15), (5, 15), (5, 15)]);
    }

    #[test]
    fn forgetting_resets_to_first_box() {
        let mut s = Schedule { level: 4, due: 10 };
        s.answer(false, 10);
        assert_eq!(s, Schedule { level: 1, due: 11 });
    }

    #[test]
    fn out_of_range_box_is_clamped() {
        let mut s = Schedule { level: 0, due: 0 };
        s.answer(true, 0);
        assert_eq!(s.level, 2);
        let mut s = Schedule { level: 99, due: 0 };
        s.answer(true, 0);
        assert_eq!(s, Schedule { level: 5, due: 15 });
    }
}
