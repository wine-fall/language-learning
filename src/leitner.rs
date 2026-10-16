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

/// Today's day number in the local time zone (UTC if the local offset
/// cannot be determined). Day numbers count days since 1970-01-01, the same
/// numbering stored in `state.json` by earlier versions.
pub fn today() -> i64 {
    let now = time::OffsetDateTime::now_local().unwrap_or_else(|_| time::OffsetDateTime::now_utc());
    day_number(now.date())
}

/// Days from 1970-01-01 to `date`.
pub fn day_number(date: time::Date) -> i64 {
    let epoch = time::OffsetDateTime::UNIX_EPOCH.date();
    i64::from(date.to_julian_day() - epoch.to_julian_day())
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
    fn day_numbers_follow_the_local_date() {
        use time::{Date, Month, OffsetDateTime, UtcOffset};
        let date = |y, m, d| Date::from_calendar_date(y, m, d).unwrap();
        assert_eq!(day_number(date(1970, Month::January, 1)), 0);
        assert_eq!(day_number(date(1969, Month::December, 31)), -1);
        assert_eq!(day_number(date(2026, Month::October, 16)), 20_742);

        // 07:30 on Oct 16 in UTC+8 is still Oct 15 in UTC: the day must
        // already have changed locally.
        let instant = OffsetDateTime::from_unix_timestamp(1_792_107_000).unwrap();
        assert_eq!(instant.unix_timestamp() / 86_400, 20_741, "UTC day");
        let east8 = instant.to_offset(UtcOffset::from_hms(8, 0, 0).unwrap());
        assert_eq!((east8.hour(), east8.minute()), (7, 30));
        assert_eq!(day_number(east8.date()), 20_742);
        let west5 = instant.to_offset(UtcOffset::from_hms(-5, 0, 0).unwrap());
        assert_eq!(day_number(west5.date()), 20_741);
    }

    #[test]
    fn today_is_within_a_day_of_utc() {
        let utc = day_number(time::OffsetDateTime::now_utc().date());
        assert!((today() - utc).abs() <= 1);
    }

    #[test]
    fn stored_day_numbers_stay_compatible() {
        // A schedule written by the UTC-based version reads back unchanged,
        // and the same day numbers are compared against the new `today`.
        let s: Schedule = serde_json::from_str(r#"{"box":2,"due":20742}"#).unwrap();
        assert_eq!(
            s,
            Schedule {
                level: 2,
                due: 20_742
            }
        );
        let today =
            day_number(time::Date::from_calendar_date(2026, time::Month::October, 16).unwrap());
        assert!(s.is_due(today));
        assert!(!s.is_due(today - 1));
        assert_eq!(
            serde_json::to_string(&s).unwrap(),
            r#"{"box":2,"due":20742}"#
        );
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
