//! Decides whether blocking is active at a given moment.
//!
//! A block belongs to the day it starts on. Blocks that run past midnight finish on the next day
//! even when that day is turned off; a turned-off day only stops new blocks from starting.

use crate::config::{Config, Track};
use chrono::{Datelike, Duration, NaiveDateTime, Timelike};

fn weekday_index(at: NaiveDateTime) -> u32 {
    at.weekday().num_days_from_monday()
}

pub fn is_blocked(config: &Config, at: NaiveDateTime) -> bool {
    let minute = (at.hour() * 60 + at.minute()) as u16;
    let mode = config.schedule_mode;
    let today = Track::for_day(mode, weekday_index(at));
    let yesterday = Track::for_day(mode, (weekday_index(at) + 6) % 7);
    let days = &config.days;

    (days.is_enabled(today) && days.get(today).blocks.iter().any(|b| b.covers_same_day(minute)))
        || (days.is_enabled(yesterday) && days.get(yesterday).blocks.iter().any(|b| b.covers_next_day(minute)))
}

/// The next minute at which the blocked/allowed state changes, looking up to eight days ahead.
pub fn next_change(config: &Config, from: NaiveDateTime) -> Option<NaiveDateTime> {
    let current = is_blocked(config, from);
    let start = from.with_second(0)?.with_nanosecond(0)?;
    (1..=8 * 1440)
        .map(|m| start + Duration::minutes(m))
        .find(|t| is_blocked(config, *t) != current)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{DaySchedule, ScheduleMode};
    use crate::time::{parse_hhmm, Block};

    fn at(text: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(text, "%Y-%m-%d %H:%M:%S").unwrap()
    }

    fn day(enabled: bool, ranges: &[(&str, &str)]) -> DaySchedule {
        DaySchedule {
            enabled,
            blocks: ranges
                .iter()
                .map(|(s, e)| Block::new(parse_hhmm(s, false).unwrap(), parse_hhmm(e, true).unwrap()).unwrap())
                .collect(),
        }
    }

    fn every_day(start: &str, end: &str) -> Config {
        let mut c = Config::default();
        c.days.every_day = day(true, &[(start, end)]);
        c
    }

    fn split(weekday_enabled: bool, weekend_enabled: bool) -> Config {
        let mut c = Config { schedule_mode: ScheduleMode::WeekdayWeekend, ..Config::default() };
        c.days.weekday = day(weekday_enabled, &[("22:00", "06:00")]);
        c.days.weekend = day(weekend_enabled, &[("12:00", "14:00")]);
        c
    }

    #[test]
    fn overnight_and_same_day_edges() {
        let night = every_day("23:00", "07:00");
        assert!(!is_blocked(&night, at("2026-01-01 22:59:59")), "before overnight block");
        assert!(is_blocked(&night, at("2026-01-01 23:00:00")), "overnight start is inclusive");
        assert!(is_blocked(&night, at("2026-01-02 00:00:00")), "blocked across midnight");
        assert!(is_blocked(&night, at("2026-01-02 06:59:59")), "just before overnight end");
        assert!(!is_blocked(&night, at("2026-01-02 07:00:00")), "overnight end is exclusive");
        let work = every_day("09:00", "17:00");
        assert!(is_blocked(&work, at("2026-01-01 12:30:00")), "same-day blocked interval");
        assert!(!is_blocked(&work, at("2026-01-01 18:00:00")), "after same-day interval");
        assert!(is_blocked(&every_day("23:00", "07:00"), at("2026-01-10 03:00:00")));
    }

    #[test]
    fn weekday_weekend() {
        let c = split(true, true);
        assert!(is_blocked(&c, at("2026-01-09 23:30:00")), "Friday uses weekday start");
        assert!(is_blocked(&c, at("2026-01-10 05:30:00")), "Saturday morning finishes Friday overnight");
        assert!(!is_blocked(&c, at("2026-01-10 06:30:00")), "Friday overnight end is respected on Saturday");
        assert!(is_blocked(&c, at("2026-01-10 12:30:00")), "Saturday same-day weekend schedule applies");
        assert!(is_blocked(&c, at("2026-01-11 13:59:59")), "Sunday uses weekend schedule");
        assert!(!is_blocked(&c, at("2026-01-12 09:00:00")), "Monday does not inherit a same-day Sunday block");

        let no_weekend = split(true, false);
        assert!(is_blocked(&no_weekend, at("2026-01-10 05:30:00")), "disabled weekend still finishes Friday overnight");
        assert!(!is_blocked(&no_weekend, at("2026-01-10 12:30:00")), "disabled weekend starts no block");

        let no_weekdays = split(false, true);
        assert!(!is_blocked(&no_weekdays, at("2026-01-09 23:30:00")), "disabled weekdays start no block");
        assert!(is_blocked(&no_weekdays, at("2026-01-10 12:30:00")), "enabled weekend still applies");
    }

    #[test]
    fn individual_days() {
        let mut c = Config { schedule_mode: ScheduleMode::IndividualDays, ..Config::default() };
        c.days.monday = day(true, &[("10:00", "11:00")]);
        c.days.tuesday = day(true, &[("12:00", "13:00")]);
        c.days.wednesday = day(false, &[("14:00", "15:00")]);
        c.days.thursday = day(true, &[("16:00", "17:00")]);
        c.days.friday = day(true, &[("22:00", "06:00")]);
        c.days.saturday = day(false, &[("12:00", "13:00")]);
        c.days.sunday = day(true, &[("18:00", "19:00")]);
        assert!(!is_blocked(&c, at("2026-01-07 14:30:00")), "disabled Wednesday starts no block");
        assert!(!is_blocked(&c, at("2026-01-07 12:30:00")), "Tuesday hours do not apply on Wednesday");
        assert!(is_blocked(&c, at("2026-01-10 05:30:00")), "Friday overnight finishes Saturday");
        assert!(!is_blocked(&c, at("2026-01-10 06:30:00")), "overnight end is respected");
        assert!(!is_blocked(&c, at("2026-01-10 12:30:00")), "disabled Saturday starts no block");
        assert!(is_blocked(&c, at("2026-01-11 18:00:00")), "Sunday block applies");
    }

    #[test]
    fn several_blocks_per_day() {
        let mut c = Config::default();
        c.days.every_day = day(true, &[("09:00", "12:00"), ("14:00", "17:30"), ("22:00", "24:00")]);
        assert!(is_blocked(&c, at("2026-01-05 10:00:00")));
        assert!(!is_blocked(&c, at("2026-01-05 13:00:00")));
        assert!(is_blocked(&c, at("2026-01-05 17:29:00")));
        assert!(!is_blocked(&c, at("2026-01-05 17:30:00")));
        assert!(is_blocked(&c, at("2026-01-05 23:59:59")), "24:00 end covers the last minute");
        assert!(!is_blocked(&c, at("2026-01-06 00:00:00")), "24:00 end does not carry over");
    }

    #[test]
    fn empty_schedule_never_blocks() {
        let mut c = Config::default();
        c.days.every_day.blocks.clear();
        assert!(!is_blocked(&c, at("2026-01-05 10:00:00")));
        assert_eq!(next_change(&c, at("2026-01-05 10:00:00")), None);
    }

    #[test]
    fn finds_next_change() {
        let c = every_day("23:00", "07:00");
        assert_eq!(next_change(&c, at("2026-01-05 20:15:42")), Some(at("2026-01-05 23:00:00")));
        assert_eq!(next_change(&c, at("2026-01-05 23:30:00")), Some(at("2026-01-06 07:00:00")));
    }
}
