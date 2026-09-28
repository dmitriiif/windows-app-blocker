//! Schedule time in the Windows time zone saved when protection is first enabled.

use crate::config::{Config, ScheduleTimeZone};
use chrono::{Datelike, Duration, Local, NaiveDate, NaiveDateTime, Timelike, Utc};
use std::sync::{Mutex, OnceLock};
use windows::Win32::Foundation::{ERROR_NO_MORE_ITEMS, ERROR_SUCCESS, SYSTEMTIME};
use windows::Win32::System::Time::{
    DYNAMIC_TIME_ZONE_INFORMATION, EnumDynamicTimeZoneInformation, GetDynamicTimeZoneInformation,
    SystemTimeToTzSpecificLocalTimeEx, TIME_ZONE_ID_INVALID,
};

fn wide_string(value: &[u16]) -> Result<String, String> {
    let end = value.iter().position(|c| *c == 0).unwrap_or(value.len());
    String::from_utf16(&value[..end]).map_err(|_| "Windows returned an invalid time zone name.".into())
}

pub fn pin(config: &mut Config) -> Result<bool, String> {
    if config.schedule_time_zone.is_some() {
        return Ok(false);
    }
    let mut info = DYNAMIC_TIME_ZONE_INFORMATION::default();
    if unsafe { GetDynamicTimeZoneInformation(&mut info) } == TIME_ZONE_ID_INVALID {
        return Err("Could not detect the Windows time zone.".into());
    }
    let id = wide_string(&info.TimeZoneKeyName)?;
    if id.is_empty() {
        return Err("Windows has no named time zone to save for the schedule.".into());
    }
    config.schedule_time_zone = Some(ScheduleTimeZone {
        id,
        daylight_time_disabled: info.DynamicDaylightTimeDisabled.0 != 0,
    });
    Ok(true)
}

fn zone_for(saved: &ScheduleTimeZone) -> Result<DYNAMIC_TIME_ZONE_INFORMATION, String> {
    static CACHE: OnceLock<Mutex<Option<(ScheduleTimeZone, DYNAMIC_TIME_ZONE_INFORMATION)>>> = OnceLock::new();
    let mut cache = CACHE.get_or_init(|| Mutex::new(None)).lock().unwrap_or_else(|e| e.into_inner());
    if let Some((key, zone)) = cache.as_ref() {
        if key == saved {
            return Ok(*zone);
        }
    }
    for index in 0..1024 {
        let mut zone = DYNAMIC_TIME_ZONE_INFORMATION::default();
        match unsafe { EnumDynamicTimeZoneInformation(index, &mut zone) } {
            result if result == ERROR_SUCCESS.0 => {
                if wide_string(&zone.TimeZoneKeyName)?.eq_ignore_ascii_case(&saved.id) {
                    *cache = Some((saved.clone(), zone));
                    return Ok(zone);
                }
            }
            result if result == ERROR_NO_MORE_ITEMS.0 => break,
            result => return Err(format!("Could not read Windows time zones (error {result}).")),
        }
    }
    Err(format!("The saved Windows time zone '{}' is unavailable.", saved.id))
}

fn in_zone(saved: &ScheduleTimeZone, utc: NaiveDateTime) -> Result<NaiveDateTime, String> {
    let zone = zone_for(saved)?;
    if saved.daylight_time_disabled {
        return utc
            .checked_sub_signed(Duration::minutes((zone.Bias as i64) + (zone.StandardBias as i64)))
            .ok_or_else(|| "Could not calculate time in the saved time zone.".into());
    }
    let input = SYSTEMTIME {
        wYear: utc.year() as u16,
        wMonth: utc.month() as u16,
        wDayOfWeek: 0,
        wDay: utc.day() as u16,
        wHour: utc.hour() as u16,
        wMinute: utc.minute() as u16,
        wSecond: utc.second() as u16,
        wMilliseconds: utc.and_utc().timestamp_subsec_millis() as u16,
    };
    let mut output = SYSTEMTIME::default();
    unsafe { SystemTimeToTzSpecificLocalTimeEx(Some(&zone), &input, &mut output) }
        .map_err(|e| format!("Could not calculate time in the saved time zone: {e}"))?;
    NaiveDate::from_ymd_opt(output.wYear.into(), output.wMonth.into(), output.wDay.into())
        .and_then(|date| date.and_hms_milli_opt(output.wHour.into(), output.wMinute.into(), output.wSecond.into(), output.wMilliseconds.into()))
        .ok_or_else(|| "Windows returned an invalid schedule time.".into())
}

pub fn now(config: &Config) -> Result<NaiveDateTime, String> {
    match &config.schedule_time_zone {
        Some(zone) => in_zone(zone, Utc::now().naive_utc()),
        None => Ok(Local::now().naive_local()),
    }
}

/// Setup can use local time before the zone is saved; the monitor handles errors by blocking.
pub fn display_now(config: &Config) -> NaiveDateTime {
    now(config).unwrap_or_else(|_| Local::now().naive_local())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_and_reuses_the_windows_time_zone() {
        let mut config = Config::default();
        assert!(pin(&mut config).unwrap());
        assert!(!pin(&mut config).unwrap());
        let actual = now(&config).unwrap();
        let expected = Local::now().naive_local();
        assert!((actual - expected).num_seconds().abs() < 60);
    }

    #[test]
    fn saved_zone_uses_its_own_daylight_rules() {
        let zone = ScheduleTimeZone { id: "Pacific Standard Time".into(), daylight_time_disabled: false };
        let winter = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap().and_hms_opt(12, 0, 0).unwrap();
        let summer = NaiveDate::from_ymd_opt(2026, 7, 15).unwrap().and_hms_opt(12, 0, 0).unwrap();
        assert_eq!(in_zone(&zone, winter).unwrap().format("%H:%M").to_string(), "04:00");
        assert_eq!(in_zone(&zone, summer).unwrap().format("%H:%M").to_string(), "05:00");
        let without_daylight = ScheduleTimeZone { daylight_time_disabled: true, ..zone.clone() };
        assert_eq!(in_zone(&without_daylight, summer).unwrap().format("%H:%M").to_string(), "04:00");

        let config = Config { schedule_time_zone: Some(zone), ..Config::default() };
        let before = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap().and_hms_opt(6, 59, 0).unwrap();
        let start = before + chrono::Duration::minutes(1);
        assert!(!crate::schedule::is_blocked(&config, in_zone(config.schedule_time_zone.as_ref().unwrap(), before).unwrap()));
        assert!(crate::schedule::is_blocked(&config, in_zone(config.schedule_time_zone.as_ref().unwrap(), start).unwrap()));
    }

    #[test]
    fn unavailable_zone_does_not_fall_back_to_windows_local_time() {
        let zone = ScheduleTimeZone { id: "No such time zone".into(), daylight_time_disabled: false };
        let utc = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap().and_hms_opt(12, 0, 0).unwrap();
        assert!(in_zone(&zone, utc).is_err());
    }
}
