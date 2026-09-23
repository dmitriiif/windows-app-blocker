//! Configuration file: types, validation, atomic saving and migration from version 2.

use crate::time::{fits, parse_hhmm, Block};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::Path;

pub const CONFIG_VERSION: u32 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScheduleMode {
    EveryDay,
    WeekdayWeekend,
    IndividualDays,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Policy {
    Always,
    Never,
    AllowedHoursOnly,
}

/// One timeline lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Track {
    EveryDay,
    Weekday,
    Weekend,
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
    Sunday,
}

impl Track {
    pub const DAYS: [Track; 7] = [
        Track::Monday,
        Track::Tuesday,
        Track::Wednesday,
        Track::Thursday,
        Track::Friday,
        Track::Saturday,
        Track::Sunday,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Track::EveryDay => "Every day",
            Track::Weekday => "Mon – Fri",
            Track::Weekend => "Sat – Sun",
            Track::Monday => "Monday",
            Track::Tuesday => "Tuesday",
            Track::Wednesday => "Wednesday",
            Track::Thursday => "Thursday",
            Track::Friday => "Friday",
            Track::Saturday => "Saturday",
            Track::Sunday => "Sunday",
        }
    }

    pub fn short_name(self) -> &'static str {
        match self {
            Track::Monday => "Mon",
            Track::Tuesday => "Tue",
            Track::Wednesday => "Wed",
            Track::Thursday => "Thu",
            Track::Friday => "Fri",
            Track::Saturday => "Sat",
            Track::Sunday => "Sun",
            other => other.name(),
        }
    }

    /// Monday = 0 … Sunday = 6.
    pub fn from_weekday_index(index: u32) -> Track {
        Track::DAYS[index as usize % 7]
    }

    /// Lanes shown for a schedule mode, in display order.
    pub fn lanes(mode: ScheduleMode) -> &'static [Track] {
        match mode {
            ScheduleMode::EveryDay => &[Track::EveryDay],
            ScheduleMode::WeekdayWeekend => &[Track::Weekday, Track::Weekend],
            ScheduleMode::IndividualDays => &Track::DAYS,
        }
    }

    /// Lane that applies on a weekday (Monday = 0) in a given mode.
    pub fn for_day(mode: ScheduleMode, weekday_index: u32) -> Track {
        match mode {
            ScheduleMode::EveryDay => Track::EveryDay,
            ScheduleMode::WeekdayWeekend if weekday_index >= 5 => Track::Weekend,
            ScheduleMode::WeekdayWeekend => Track::Weekday,
            ScheduleMode::IndividualDays => Track::from_weekday_index(weekday_index),
        }
    }

    /// Lanes whose overnight blocks carry over into the start of `self`.
    pub fn carry_sources(self) -> &'static [Track] {
        match self {
            Track::EveryDay => &[Track::EveryDay],
            Track::Weekday => &[Track::Weekday, Track::Weekend],
            Track::Weekend => &[Track::Weekend, Track::Weekday],
            Track::Monday => &[Track::Sunday],
            Track::Tuesday => &[Track::Monday],
            Track::Wednesday => &[Track::Tuesday],
            Track::Thursday => &[Track::Wednesday],
            Track::Friday => &[Track::Thursday],
            Track::Saturday => &[Track::Friday],
            Track::Sunday => &[Track::Saturday],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DaySchedule {
    pub enabled: bool,
    pub blocks: Vec<Block>,
}

impl DaySchedule {
    fn with(start: u16, end: u16) -> DaySchedule {
        DaySchedule { enabled: true, blocks: vec![Block::new(start, end).expect("valid default")] }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Days {
    pub every_day: DaySchedule,
    pub weekday: DaySchedule,
    pub weekend: DaySchedule,
    pub monday: DaySchedule,
    pub tuesday: DaySchedule,
    pub wednesday: DaySchedule,
    pub thursday: DaySchedule,
    pub friday: DaySchedule,
    pub saturday: DaySchedule,
    pub sunday: DaySchedule,
}

impl Days {
    pub fn get(&self, track: Track) -> &DaySchedule {
        match track {
            Track::EveryDay => &self.every_day,
            Track::Weekday => &self.weekday,
            Track::Weekend => &self.weekend,
            Track::Monday => &self.monday,
            Track::Tuesday => &self.tuesday,
            Track::Wednesday => &self.wednesday,
            Track::Thursday => &self.thursday,
            Track::Friday => &self.friday,
            Track::Saturday => &self.saturday,
            Track::Sunday => &self.sunday,
        }
    }

    pub fn get_mut(&mut self, track: Track) -> &mut DaySchedule {
        match track {
            Track::EveryDay => &mut self.every_day,
            Track::Weekday => &mut self.weekday,
            Track::Weekend => &mut self.weekend,
            Track::Monday => &mut self.monday,
            Track::Tuesday => &mut self.tuesday,
            Track::Wednesday => &mut self.wednesday,
            Track::Thursday => &mut self.thursday,
            Track::Friday => &mut self.friday,
            Track::Saturday => &mut self.saturday,
            Track::Sunday => &mut self.sunday,
        }
    }

    /// Whether a lane can start blocks. "Every day" has no off switch.
    pub fn is_enabled(&self, track: Track) -> bool {
        track == Track::EveryDay || self.get(track).enabled
    }
}

impl Default for Days {
    fn default() -> Days {
        let night = DaySchedule::with(23 * 60, 7 * 60);
        let weekend = DaySchedule::with(0, 9 * 60);
        Days {
            every_day: night.clone(),
            weekday: night.clone(),
            weekend: weekend.clone(),
            monday: night.clone(),
            tuesday: night.clone(),
            wednesday: night.clone(),
            thursday: night.clone(),
            friday: night,
            saturday: weekend.clone(),
            sunday: weekend,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Policies {
    pub change_times: Policy,
    pub turn_off: Policy,
    pub uninstall: Policy,
    pub remove_executables: Policy,
    /// Whether these choices can be edited later from Settings. Configurations written before
    /// this option existed get `Never`, like new installs.
    #[serde(default = "never")]
    pub change_locks: Policy,
}

fn never() -> Policy {
    Policy::Never
}

impl Default for Policies {
    fn default() -> Policies {
        Policies {
            change_times: Policy::Always,
            turn_off: Policy::Always,
            uninstall: Policy::Always,
            remove_executables: Policy::Always,
            change_locks: Policy::Never,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Shortcuts {
    pub start_menu: bool,
    pub desktop: bool,
}

impl Default for Shortcuts {
    fn default() -> Shortcuts {
        Shortcuts { start_menu: true, desktop: false }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Theme {
    #[default]
    Violet,
    Blue,
    Gold,
    Light,
}

impl Theme {
    pub const ALL: [Theme; 4] = [Theme::Violet, Theme::Blue, Theme::Gold, Theme::Light];

    pub fn name(self) -> &'static str {
        match self {
            Theme::Violet => "Violet",
            Theme::Blue => "Blue",
            Theme::Gold => "Gold",
            Theme::Light => "Light",
        }
    }
}

/// Personal settings. Not covered by the lock choices made during setup.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub theme: Theme,
    pub notify_before_block: bool,
    pub notify_minutes: u32,
}

impl Default for Preferences {
    fn default() -> Preferences {
        Preferences { theme: Theme::default(), notify_before_block: false, notify_minutes: 10 }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    pub version: u32,
    pub setup_completed: bool,
    pub schedule_mode: ScheduleMode,
    pub days: Days,
    pub policies: Policies,
    pub executables: Vec<String>,
    pub target_user_sid: String,
    pub check_interval_seconds: u32,
    #[serde(default)]
    pub shortcuts: Shortcuts,
    #[serde(default)]
    pub preferences: Preferences,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            version: CONFIG_VERSION,
            setup_completed: false,
            schedule_mode: ScheduleMode::EveryDay,
            days: Days::default(),
            policies: Policies::default(),
            executables: Vec::new(),
            target_user_sid: String::new(),
            check_interval_seconds: 2,
            shortcuts: Shortcuts::default(),
            preferences: Preferences::default(),
        }
    }
}

impl Config {
    /// Checks everything except the target SID, which is only known once installed.
    pub fn validate_schedule(&self) -> Result<(), String> {
        let all = [Track::EveryDay, Track::Weekday, Track::Weekend]
            .into_iter()
            .chain(Track::DAYS);
        for track in all {
            let blocks = &self.days.get(track).blocks;
            for (index, block) in blocks.iter().enumerate() {
                if !fits(&blocks[index + 1..], None, block) {
                    return Err(format!("Two blocks overlap on {}.", track.name()));
                }
            }
        }
        Ok(())
    }

    pub fn validate(&self) -> Result<(), String> {
        self.validate_schedule()?;
        if !(1..=60).contains(&self.check_interval_seconds) {
            return Err("check_interval_seconds must be between 1 and 60.".into());
        }
        if !(1..=60).contains(&self.preferences.notify_minutes) {
            return Err("notify_minutes must be between 1 and 60.".into());
        }
        if self.target_user_sid.trim().is_empty() {
            return Err("target_user_sid is required.".into());
        }
        if !is_valid_sid(&self.target_user_sid) {
            return Err("target_user_sid is not a valid Windows security identifier.".into());
        }
        for executable in &self.executables {
            validate_executable(executable)?;
        }
        Ok(())
    }
}

pub fn validate_executable(path: &str) -> Result<(), String> {
    let p = Path::new(path);
    let is_exe = p.extension().is_some_and(|e| e.eq_ignore_ascii_case("exe"));
    if !p.is_absolute() || !is_exe {
        return Err(format!("Invalid executable path '{path}'. Choose an absolute .exe path."));
    }
    // `*` is allowed in folder names only (see `paths::is_configured`); the file name must be exact.
    let name = p.file_name().map(|n| n.to_string_lossy()).unwrap_or_default();
    if path.contains('?') || name.contains('*') {
        return Err(format!("Invalid executable path '{path}'. Only folder names may contain '*'."));
    }
    Ok(())
}

/// Checks the textual form `S-1-<authority>-<sub>…` of a Windows SID.
pub fn is_valid_sid(sid: &str) -> bool {
    let mut parts = sid.split('-');
    if !matches!(parts.next(), Some("S" | "s")) || parts.next() != Some("1") {
        return false;
    }
    let Some(authority) = parts.next() else { return false };
    if authority.parse::<u64>().is_err() {
        return false;
    }
    let subs: Vec<&str> = parts.collect();
    (1..=15).contains(&subs.len()) && subs.iter().all(|s| s.parse::<u32>().is_ok())
}

/// Parses a configuration of any supported version and validates it.
pub fn from_json_str(text: &str) -> Result<Config, String> {
    let text = text.trim_start_matches('\u{feff}');
    let value: Value = serde_json::from_str(text).map_err(|e| format!("The configuration is not valid JSON: {e}"))?;
    let config = if value.get("version").and_then(Value::as_u64).unwrap_or(0) >= 3 {
        serde_json::from_value(value).map_err(|e| format!("The configuration is invalid: {e}"))?
    } else {
        migrate_v2(&value)?
    };
    config.validate()?;
    Ok(config)
}

/// Whether a configuration file was written by the previous (version 2) app.
pub fn is_legacy_json(text: &str) -> bool {
    serde_json::from_str::<Value>(text.trim_start_matches('\u{feff}'))
        .map(|v| v.get("version").is_none())
        .unwrap_or(false)
}

pub fn load(path: &Path) -> Result<Config, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("Could not read {}: {e}", path.display()))?;
    from_json_str(&text)
}

/// Writes the configuration through a validated temporary file, then replaces the original.
pub fn save(config: &Config, path: &Path) -> Result<(), String> {
    let mut config = config.clone();
    config.version = CONFIG_VERSION;
    let json = serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?;
    from_json_str(&json)?;
    let parent = path.parent().ok_or("The configuration path has no folder.")?;
    fs::create_dir_all(parent).map_err(|e| format!("Could not create {}: {e}", parent.display()))?;
    let temporary = parent.join(format!(".config-{}.tmp", std::process::id()));
    let result = fs::write(&temporary, json)
        .and_then(|_| fs::rename(&temporary, path))
        .map_err(|e| format!("Could not save the configuration: {e}"));
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn migrate_v2(v: &Value) -> Result<Config, String> {
    let text = |key: &str| v.get(key).and_then(Value::as_str).map(str::to_owned);
    let flag = |key: &str| v.get(key).and_then(Value::as_bool).unwrap_or(true);
    let policy = |key: &str| -> Result<Policy, String> {
        match text(key).as_deref() {
            None | Some("Always") => Ok(Policy::Always),
            Some("Never") => Ok(Policy::Never),
            Some("AllowedHoursOnly") => Ok(Policy::AllowedHoursOnly),
            Some(_) => Err(format!("{key} has an invalid value.")),
        }
    };

    let block_start = text("BlockStart").unwrap_or_else(|| "23:00".into());
    let block_end = text("BlockEnd").unwrap_or_else(|| "07:00".into());
    let weekday_start = text("WeekdayBlockStart").unwrap_or_else(|| block_start.clone());
    let weekday_end = text("WeekdayBlockEnd").unwrap_or_else(|| block_end.clone());
    let weekend_start = text("WeekendBlockStart").unwrap_or_else(|| block_start.clone());
    let weekend_end = text("WeekendBlockEnd").unwrap_or_else(|| block_end.clone());

    let day = |name: &str, start: String, end: String, enabled: bool| -> Result<DaySchedule, String> {
        let s = parse_hhmm(&start, false)?;
        let e = parse_hhmm(&end, false)?;
        if s == e {
            if enabled {
                return Err(format!("{name} start and end times must be different."));
            }
            return Ok(DaySchedule { enabled, blocks: Vec::new() });
        }
        Ok(DaySchedule { enabled, blocks: vec![Block::new(s, e)?] })
    };

    let individual = |prefix: &str, weekend: bool| -> Result<DaySchedule, String> {
        let (ds, de) = if weekend {
            (weekend_start.clone(), weekend_end.clone())
        } else {
            (weekday_start.clone(), weekday_end.clone())
        };
        day(
            prefix,
            text(&format!("{prefix}BlockStart")).unwrap_or(ds),
            text(&format!("{prefix}BlockEnd")).unwrap_or(de),
            flag(&format!("{prefix}Enabled")),
        )
    };

    let schedule_mode = match text("ScheduleMode").as_deref() {
        None | Some("EveryDay") => ScheduleMode::EveryDay,
        Some("WeekdayWeekend") => ScheduleMode::WeekdayWeekend,
        Some("IndividualDays") => ScheduleMode::IndividualDays,
        Some(_) => return Err("ScheduleMode must be 'EveryDay', 'WeekdayWeekend', or 'IndividualDays'.".into()),
    };

    let executables = match v.get("Executables") {
        Some(Value::Array(items)) => items.iter().filter_map(Value::as_str).map(str::to_owned).collect(),
        Some(Value::String(single)) => vec![single.clone()],
        _ => Vec::new(),
    };

    let setup_version = v.get("SetupVersion").and_then(Value::as_u64).unwrap_or(0);
    let setup_completed = v.get("SetupCompleted").and_then(Value::as_bool).unwrap_or(false) && setup_version >= 2;

    Ok(Config {
        version: CONFIG_VERSION,
        setup_completed,
        schedule_mode,
        days: Days {
            every_day: day("Every-day", block_start.clone(), block_end.clone(), true)?,
            weekday: day("Weekday", weekday_start.clone(), weekday_end.clone(), flag("WeekdayEnabled"))?,
            weekend: day("Weekend", weekend_start.clone(), weekend_end.clone(), flag("WeekendEnabled"))?,
            monday: individual("Monday", false)?,
            tuesday: individual("Tuesday", false)?,
            wednesday: individual("Wednesday", false)?,
            thursday: individual("Thursday", false)?,
            friday: individual("Friday", false)?,
            saturday: individual("Saturday", true)?,
            sunday: individual("Sunday", true)?,
        },
        policies: Policies {
            change_times: policy("ChangeTimesPolicy")?,
            turn_off: policy("TurnOffProtectionPolicy")?,
            uninstall: policy("UninstallPolicy")?,
            remove_executables: policy("RemoveExecutablesPolicy")?,
            change_locks: Policy::Never,
        },
        executables,
        target_user_sid: text("TargetUserSid").unwrap_or_default(),
        check_interval_seconds: v.get("CheckIntervalSeconds").and_then(Value::as_u64).unwrap_or(2) as u32,
        shortcuts: Shortcuts::default(),
        preferences: Preferences::default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const SID: &str = "S-1-5-21-1000-1000-1000-1000";

    fn legacy(extra: Value) -> String {
        let mut base = json!({
            "BlockStart": "23:00", "BlockEnd": "07:00",
            "CheckIntervalSeconds": 2, "TargetUserSid": SID,
            "Executables": ["C:\\Program Files\\Example\\Example.exe", "D:\\Tools\\Editor.exe"]
        });
        for (k, v) in extra.as_object().unwrap() {
            base[k] = v.clone();
        }
        base.to_string()
    }

    #[test]
    fn legacy_config_receives_defaults() {
        let c = from_json_str(&legacy(json!({}))).unwrap();
        assert_eq!(c.executables.len(), 2, "valid executable list loads");
        assert_eq!(c.schedule_mode, ScheduleMode::EveryDay);
        assert_eq!(c.policies, Policies::default(), "permissive policy defaults");
        assert_eq!(c.days.monday.blocks, vec![Block::new(1380, 420).unwrap()]);
        assert!(c.days.weekday.enabled && c.days.weekend.enabled && c.days.monday.enabled);
        assert!(!c.setup_completed);
    }

    #[test]
    fn legacy_config_migrates_every_field() {
        let c = from_json_str(&legacy(json!({
            "ScheduleMode": "IndividualDays",
            "WednesdayBlockStart": "14:00", "WednesdayBlockEnd": "15:00", "WednesdayEnabled": false,
            "SaturdayBlockStart": "00:00", "SaturdayBlockEnd": "09:00",
            "ChangeTimesPolicy": "Never", "UninstallPolicy": "AllowedHoursOnly",
            "SetupCompleted": true, "SetupVersion": 2
        })))
        .unwrap();
        assert_eq!(c.schedule_mode, ScheduleMode::IndividualDays);
        assert!(!c.days.wednesday.enabled);
        assert_eq!(c.days.wednesday.blocks, vec![Block::new(840, 900).unwrap()]);
        assert_eq!(c.days.saturday.blocks, vec![Block::new(0, 540).unwrap()]);
        assert_eq!(c.policies.change_times, Policy::Never);
        assert_eq!(c.policies.uninstall, Policy::AllowedHoursOnly);
        assert!(c.setup_completed);
    }

    #[test]
    fn disabled_day_does_not_require_a_range() {
        let text = legacy(json!({
            "ScheduleMode": "IndividualDays",
            "MondayBlockStart": "10:00", "MondayBlockEnd": "10:00", "MondayEnabled": false,
            "Executables": []
        }));
        assert!(from_json_str(&text).unwrap().days.monday.blocks.is_empty());
    }

    #[test]
    fn enabled_equal_range_is_rejected() {
        assert!(from_json_str(&legacy(json!({ "BlockStart": "08:00", "BlockEnd": "08:00" }))).is_err());
    }

    #[test]
    fn invalid_values_are_rejected() {
        assert!(from_json_str(&legacy(json!({ "TargetUserSid": "not-a-sid" }))).is_err(), "bad SID");
        assert!(from_json_str(&legacy(json!({ "Executables": ["relative-program.exe"] }))).is_err(), "relative exe");
        assert!(from_json_str(&legacy(json!({ "Executables": ["C:\\Tools\\notes.txt"] }))).is_err(), "not an exe");
        assert!(from_json_str(&legacy(json!({ "Executables": ["C:\\Tools\\*.exe"] }))).is_err(), "wildcard file name");
        assert!(from_json_str(&legacy(json!({ "Executables": ["C:\\Tools\\app-*\\Tool.exe"] }))).is_ok(), "wildcard folder");
        assert!(from_json_str(&legacy(json!({ "ChangeTimesPolicy": "Sometimes" }))).is_err(), "bad policy");
        assert!(from_json_str(&legacy(json!({ "CheckIntervalSeconds": 0 }))).is_err(), "bad interval");
        assert!(from_json_str(&legacy(json!({ "BlockStart": "25:00" }))).is_err(), "bad time");
    }

    #[test]
    fn round_trips_and_rejects_overlap() {
        let mut c = Config { target_user_sid: SID.into(), ..Config::default() };
        c.days.monday.blocks = vec![Block::new(600, 660).unwrap(), Block::new(1320, 1440).unwrap()];
        let text = serde_json::to_string(&c).unwrap();
        assert!(text.contains("\"24:00\""));
        assert_eq!(from_json_str(&text).unwrap(), c);

        c.days.monday.blocks.push(Block::new(630, 700).unwrap());
        assert!(from_json_str(&serde_json::to_string(&c).unwrap()).is_err());
    }

    #[test]
    fn lock_choices_are_locked_by_default() {
        let mut c = Config { target_user_sid: SID.into(), ..Config::default() };
        assert_eq!(c.policies.change_locks, Policy::Never);
        c.policies.change_locks = Policy::Always;
        let mut value = serde_json::to_value(&c).unwrap();
        value["policies"].as_object_mut().unwrap().remove("change_locks");
        assert_eq!(from_json_str(&value.to_string()).unwrap().policies.change_locks, Policy::Never, "older v3 file");
    }

    #[test]
    fn sid_format() {
        assert!(is_valid_sid(SID));
        assert!(is_valid_sid("S-1-5-18"));
        assert!(!is_valid_sid("S-1-5"));
        assert!(!is_valid_sid("S-2-5-18"));
        assert!(!is_valid_sid("INSTALLER_WILL_SET_THIS"));
    }

    #[test]
    fn save_and_load() {
        let dir = std::env::temp_dir().join(format!("app-blocker-test-{}", std::process::id()));
        let path = dir.join("config.json");
        let c = Config { target_user_sid: SID.into(), ..Config::default() };
        save(&c, &path).unwrap();
        save(&c, &path).unwrap();
        assert_eq!(load(&path).unwrap(), c);
        let _ = fs::remove_dir_all(dir);
    }
}
