//! Minutes-of-day and blocks on a circular 24-hour day.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub const DAY: u16 = 1440;

/// Parses `HH:MM` (24-hour). `24:00` is accepted only when `allow_midnight_end` is set.
pub fn parse_hhmm(value: &str, allow_midnight_end: bool) -> Result<u16, String> {
    let invalid = || format!("Invalid time '{value}'. Use 24-hour HH:MM format.");
    let (h, m) = value.trim().split_once(':').ok_or_else(invalid)?;
    if h.is_empty() || h.len() > 2 || m.len() != 2 {
        return Err(invalid());
    }
    let h: u16 = h.parse().map_err(|_| invalid())?;
    let m: u16 = m.parse().map_err(|_| invalid())?;
    if m > 59 {
        return Err(invalid());
    }
    if h == 24 && m == 0 && allow_midnight_end {
        return Ok(DAY);
    }
    if h > 23 {
        return Err(invalid());
    }
    Ok(h * 60 + m)
}

pub fn format_hhmm(minute: u16) -> String {
    format!("{:02}:{:02}", minute / 60, minute % 60)
}

pub fn format_duration(minutes: u32) -> String {
    match (minutes / 60, minutes % 60) {
        (0, m) => format!("{m}m"),
        (h, 0) => format!("{h}h"),
        (h, m) => format!("{h}h {m}m"),
    }
}

/// A blocking period. `start` is included and `end` is not.
///
/// `start` is in `0..1440`. `end` is in `1..=1440`, where `1440` means midnight at the end of the
/// day. When `end < start` the block runs past midnight and finishes on the next day.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Block {
    pub start: u16,
    pub end: u16,
}

impl Block {
    /// Builds a block, normalising an end of `00:00` to `24:00`.
    pub fn new(start: u16, end: u16) -> Result<Block, String> {
        let end = if end == 0 { DAY } else { end };
        if start >= DAY || end > DAY {
            return Err("Times must be between 00:00 and 24:00.".into());
        }
        if start == end {
            return Err("A block's start and end times must be different.".into());
        }
        Ok(Block { start, end })
    }

    /// Builds a block from a start and a length, clamping to a valid shape.
    pub fn from_start_len(start: i32, len: i32) -> Block {
        let start = start.clamp(0, DAY as i32 - 1);
        let max_len = if start == 0 { DAY as i32 } else { DAY as i32 - 1 };
        let len = len.clamp(1, max_len);
        let mut end = start + len;
        if end > DAY as i32 {
            end -= DAY as i32;
        }
        Block { start: start as u16, end: end as u16 }
    }

    pub fn is_overnight(&self) -> bool {
        self.end < self.start
    }

    /// Length in minutes.
    pub fn minutes(&self) -> u16 {
        if self.end > self.start {
            self.end - self.start
        } else {
            self.end + DAY - self.start
        }
    }

    /// End measured from the start of the day the block begins on (may exceed 1440).
    pub fn end_abs(&self) -> u16 {
        self.start + self.minutes()
    }

    /// Whether the block covers `minute` on the day it starts.
    pub fn covers_same_day(&self, minute: u16) -> bool {
        minute >= self.start && (self.is_overnight() || minute < self.end)
    }

    /// Whether the block's carry-over covers `minute` on the following day.
    pub fn covers_next_day(&self, minute: u16) -> bool {
        self.is_overnight() && minute < self.end
    }

    /// Whether two blocks share any minute on a circular day.
    pub fn overlaps(&self, other: &Block) -> bool {
        let forward = (other.start + DAY - self.start) % DAY;
        let backward = (self.start + DAY - other.start) % DAY;
        forward < self.minutes() || backward < other.minutes()
    }

    pub fn label(&self) -> String {
        format!("{} – {}", format_hhmm(self.start), format_hhmm(self.end))
    }
}

/// Whether `candidate` fits among `blocks` without overlapping, ignoring index `skip`.
pub fn fits(blocks: &[Block], skip: Option<usize>, candidate: &Block) -> bool {
    blocks
        .iter()
        .enumerate()
        .filter(|(i, _)| Some(*i) != skip)
        .all(|(_, b)| !b.overlaps(candidate))
}

impl Serialize for Block {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        struct Raw {
            start: String,
            end: String,
        }
        Raw { start: format_hhmm(self.start), end: format_hhmm(self.end) }.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Block {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Raw {
            start: String,
            end: String,
        }
        let raw = Raw::deserialize(deserializer)?;
        let start = parse_hhmm(&raw.start, false).map_err(serde::de::Error::custom)?;
        let end = parse_hhmm(&raw.end, true).map_err(serde::de::Error::custom)?;
        Block::new(start, end).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn b(s: &str, e: &str) -> Block {
        Block::new(parse_hhmm(s, false).unwrap(), parse_hhmm(e, true).unwrap()).unwrap()
    }

    #[test]
    fn parses_times() {
        assert_eq!(parse_hhmm("07:05", false), Ok(425));
        assert_eq!(parse_hhmm("24:00", true), Ok(1440));
        assert!(parse_hhmm("24:00", false).is_err());
        assert!(parse_hhmm("25:00", true).is_err(), "invalid time is rejected");
        assert!(parse_hhmm("12:60", false).is_err());
        assert!(parse_hhmm("1200", false).is_err());
        assert_eq!(format_hhmm(1440), "24:00");
    }

    #[test]
    fn equal_start_and_end_is_rejected() {
        assert!(Block::new(480, 480).is_err());
    }

    #[test]
    fn midnight_end_is_normalised() {
        assert_eq!(Block::new(1380, 0).unwrap(), Block { start: 1380, end: 1440 });
        assert!(!b("23:00", "00:00").is_overnight());
    }

    #[test]
    fn circular_overlap() {
        assert!(b("09:00", "10:00").overlaps(&b("09:30", "11:00")));
        assert!(!b("09:00", "10:00").overlaps(&b("10:00", "11:00")));
        assert!(b("22:00", "02:00").overlaps(&b("01:00", "03:00")));
        assert!(!b("22:00", "02:00").overlaps(&b("02:00", "03:00")));
        assert!(b("00:00", "24:00").overlaps(&b("12:00", "13:00")));
    }

    #[test]
    fn start_len_wraps() {
        assert_eq!(Block::from_start_len(1380, 120), b("23:00", "01:00"));
        assert_eq!(Block::from_start_len(1380, 60), b("23:00", "24:00"));
        assert_eq!(Block::from_start_len(0, 5000), b("00:00", "24:00"));
        assert_eq!(Block::from_start_len(60, 5000).minutes(), 1439);
    }
}
