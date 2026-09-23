//! Setup-time choices that decide which controls stay available later.

use crate::config::{Config, Policy};
use crate::schedule::is_blocked;
use chrono::NaiveDateTime;

pub fn can_use(policy: Policy, config: &Config, now: NaiveDateTime) -> bool {
    match policy {
        Policy::Always => true,
        Policy::Never => false,
        Policy::AllowedHoursOnly => !is_blocked(config, now),
    }
}

pub fn display(policy: Policy) -> &'static str {
    match policy {
        Policy::Always => "anytime",
        Policy::Never => "never",
        Policy::AllowedHoursOnly => "allowed hours only",
    }
}

pub fn choice_label(policy: Policy) -> &'static str {
    match policy {
        Policy::Always => "Yes — anytime",
        Policy::Never => "No — never",
        Policy::AllowedHoursOnly => "Only during allowed hours",
    }
}

pub fn unavailable_message(policy: Policy, subject: &str) -> String {
    if policy == Policy::Never {
        format!("{subject} because you chose ‘No — never’ during setup.")
    } else {
        format!("{subject} during blocked hours. Try again during allowed hours.")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDateTime;

    fn at(text: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(text, "%Y-%m-%d %H:%M").unwrap()
    }

    #[test]
    fn policies_follow_the_schedule() {
        let config = Config::default(); // 23:00–07:00 every day
        let blocked = at("2026-01-05 23:30");
        let allowed = at("2026-01-05 12:00");
        assert!(can_use(Policy::Always, &config, blocked));
        assert!(!can_use(Policy::Never, &config, allowed));
        assert!(can_use(Policy::AllowedHoursOnly, &config, allowed));
        assert!(!can_use(Policy::AllowedHoursOnly, &config, blocked));
    }
}
