//! Finds commonly blocked apps (browsers, game launchers, chat) installed on this computer.
//!
//! Three sources, in order: the usual install folders, the registry's App Paths (and a few
//! app-specific keys), and the programs running right now, which catches installs on other drives.
//! Apps that install each update into a new versioned folder are suggested as a `*` path, so the
//! block keeps working after they update.

use crate::paths;
use crate::win;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Category {
    Browsers,
    Games,
    Chat,
    Music,
}

impl Category {
    pub const ALL: [Category; 4] = [Category::Browsers, Category::Games, Category::Chat, Category::Music];

    pub fn name(self) -> &'static str {
        match self {
            Category::Browsers => "Web browsers",
            Category::Games => "Games and game stores",
            Category::Chat => "Chat and social",
            Category::Music => "Music and video",
        }
    }
}

struct Known {
    name: &'static str,
    category: Category,
    /// Paths starting with `%Variable%` (an environment variable, or `%Desktop%`). `*` marks a
    /// versioned folder.
    locations: &'static [&'static str],
    /// `HKEY_CURRENT_USER` values holding the path of the program.
    user_registry: &'static [(&'static str, &'static str)],
}

const fn app(name: &'static str, category: Category, locations: &'static [&'static str]) -> Known {
    Known { name, category, locations, user_registry: &[] }
}

static CATALOG: &[Known] = &[
    // Browsers. Blocking Edge leaves WebView2 (msedgewebview2.exe), which Windows and other apps use, alone.
    app("Google Chrome", Category::Browsers, &[
        r"%ProgramFiles%\Google\Chrome\Application\chrome.exe",
        r"%ProgramFiles(x86)%\Google\Chrome\Application\chrome.exe",
        r"%LocalAppData%\Google\Chrome\Application\chrome.exe",
    ]),
    app("Microsoft Edge", Category::Browsers, &[
        r"%ProgramFiles(x86)%\Microsoft\Edge\Application\msedge.exe",
        r"%ProgramFiles%\Microsoft\Edge\Application\msedge.exe",
    ]),
    app("Mozilla Firefox", Category::Browsers, &[
        r"%ProgramFiles%\Mozilla Firefox\firefox.exe",
        r"%ProgramFiles(x86)%\Mozilla Firefox\firefox.exe",
        r"%LocalAppData%\Mozilla Firefox\firefox.exe",
    ]),
    app("Brave", Category::Browsers, &[
        r"%ProgramFiles%\BraveSoftware\Brave-Browser\Application\brave.exe",
        r"%ProgramFiles(x86)%\BraveSoftware\Brave-Browser\Application\brave.exe",
        r"%LocalAppData%\BraveSoftware\Brave-Browser\Application\brave.exe",
    ]),
    app("Opera", Category::Browsers, &[
        r"%LocalAppData%\Programs\Opera\opera.exe",
        r"%LocalAppData%\Programs\Opera\*\opera.exe",
        r"%ProgramFiles%\Opera\opera.exe",
        r"%ProgramFiles%\Opera\*\opera.exe",
    ]),
    app("Opera GX", Category::Browsers, &[
        r"%LocalAppData%\Programs\Opera GX\opera.exe",
        r"%LocalAppData%\Programs\Opera GX\*\opera.exe",
    ]),
    app("Vivaldi", Category::Browsers, &[
        r"%LocalAppData%\Vivaldi\Application\vivaldi.exe",
        r"%ProgramFiles%\Vivaldi\Application\vivaldi.exe",
    ]),
    app("Tor Browser", Category::Browsers, &[r"%Desktop%\Tor Browser\Browser\firefox.exe"]),
    // Games and game stores.
    Known {
        name: "Steam",
        category: Category::Games,
        locations: &[r"%ProgramFiles(x86)%\Steam\steam.exe", r"%ProgramFiles%\Steam\steam.exe"],
        user_registry: &[(r"Software\Valve\Steam", "SteamExe")],
    },
    app("Epic Games Launcher", Category::Games, &[
        r"%ProgramFiles(x86)%\Epic Games\Launcher\Portal\Binaries\Win64\EpicGamesLauncher.exe",
        r"%ProgramFiles(x86)%\Epic Games\Launcher\Portal\Binaries\Win32\EpicGamesLauncher.exe",
        r"%ProgramFiles%\Epic Games\Launcher\Portal\Binaries\Win64\EpicGamesLauncher.exe",
    ]),
    app("Fortnite", Category::Games, &[
        r"%ProgramFiles%\Epic Games\Fortnite\FortniteGame\Binaries\Win64\FortniteClient-Win64-Shipping.exe",
    ]),
    app("Battle.net", Category::Games, &[
        r"%ProgramFiles(x86)%\Battle.net\Battle.net.exe",
        r"%ProgramFiles(x86)%\Battle.net\Battle.net Launcher.exe",
    ]),
    app("EA app", Category::Games, &[r"%ProgramFiles%\Electronic Arts\EA Desktop\EA Desktop\EADesktop.exe"]),
    app("Ubisoft Connect", Category::Games, &[
        r"%ProgramFiles(x86)%\Ubisoft\Ubisoft Game Launcher\UbisoftConnect.exe",
        r"%ProgramFiles(x86)%\Ubisoft\Ubisoft Game Launcher\upc.exe",
    ]),
    app("GOG Galaxy", Category::Games, &[r"%ProgramFiles(x86)%\GOG Galaxy\GalaxyClient.exe"]),
    app("Riot Client", Category::Games, &[r"%SystemDrive%\Riot Games\Riot Client\RiotClientServices.exe"]),
    app("League of Legends", Category::Games, &[
        r"%SystemDrive%\Riot Games\League of Legends\LeagueClient.exe",
        r"%SystemDrive%\Riot Games\League of Legends\Game\League of Legends.exe",
    ]),
    app("VALORANT", Category::Games, &[r"%SystemDrive%\Riot Games\VALORANT\live\VALORANT.exe"]),
    app("Roblox", Category::Games, &[
        r"%LocalAppData%\Roblox\Versions\*\RobloxPlayerBeta.exe",
        r"%ProgramFiles(x86)%\Roblox\Versions\*\RobloxPlayerBeta.exe",
    ]),
    app("Minecraft Launcher", Category::Games, &[r"%ProgramFiles(x86)%\Minecraft Launcher\MinecraftLauncher.exe"]),
    // Chat and social.
    app("Discord", Category::Chat, &[r"%LocalAppData%\Discord\app-*\Discord.exe"]),
    app("Discord PTB", Category::Chat, &[r"%LocalAppData%\DiscordPTB\app-*\DiscordPTB.exe"]),
    app("Discord Canary", Category::Chat, &[r"%LocalAppData%\DiscordCanary\app-*\DiscordCanary.exe"]),
    app("Telegram", Category::Chat, &[r"%AppData%\Telegram Desktop\Telegram.exe"]),
    app("WhatsApp", Category::Chat, &[r"%ProgramFiles%\WindowsApps\5319275A.WhatsAppDesktop_*\WhatsApp.exe"]),
    app("Slack", Category::Chat, &[r"%LocalAppData%\slack\app-*\slack.exe", r"%ProgramFiles%\Slack\slack.exe"]),
    // Music and video.
    app("Spotify", Category::Music, &[
        r"%AppData%\Spotify\Spotify.exe",
        r"%ProgramFiles%\WindowsApps\SpotifyAB.SpotifyMusic_*\Spotify.exe",
    ]),
];

/// An app found on this computer.
#[derive(Clone, Debug)]
pub struct Found {
    pub name: &'static str,
    pub category: Category,
    /// What to add to the block list; may contain `*` in folder names.
    pub path: String,
    /// Every copy of this app is already covered by the block list.
    pub already_added: bool,
}

/// Looks for the apps in the catalog. `existing` is the current block list.
pub fn scan(existing: &[String]) -> Vec<Found> {
    let mut found: Vec<(usize, String)> = Vec::new();
    fn add(index: usize, path: String, found: &mut Vec<(usize, String)>) {
        let known: Vec<String> = found.iter().map(|(_, p)| p.clone()).collect();
        if !covers(&known, &path) {
            found.push((index, path));
        }
    }

    for (index, known) in CATALOG.iter().enumerate() {
        for location in known.locations {
            let Some(path) = expand_vars(location) else { continue };
            if !paths::expand(&path).is_empty() {
                add(index, path, &mut found);
            }
        }
        for path in registry_paths(known) {
            add(index, path, &mut found);
        }
    }

    // Running programs catch installs in other places, e.g. Steam on a games drive.
    if let Ok(processes) = win::list_processes() {
        for process in processes {
            let Some(index) = catalog_index_for(&process.name) else { continue };
            let Some(path) = win::OpenedProcess::open(process.pid).and_then(|p| p.image_path()) else { continue };
            add(index, path, &mut found);
        }
    }

    let mut result: Vec<Found> = found
        .into_iter()
        .map(|(index, path)| {
            let known = &CATALOG[index];
            let already_added = is_already_added(existing, &path);
            Found { name: known.name, category: known.category, path, already_added }
        })
        .collect();
    result.sort_by_key(|f| (f.category, CATALOG.iter().position(|k| k.name == f.name)));
    result
}

/// Whether `path` (maybe a pattern) is equal to, or matched by, one of `list`.
fn covers(list: &[String], path: &str) -> bool {
    list.iter().any(|p| paths::same_path(p, path)) || (!path.contains('*') && paths::is_configured(Some(path), list))
}

fn is_already_added(existing: &[String], path: &str) -> bool {
    if covers(existing, path) {
        return true;
    }
    let copies = paths::expand(path);
    !copies.is_empty() && copies.iter().all(|c| paths::is_configured(Some(&c.to_string_lossy()), existing))
}

/// The catalog entry whose program has this file name, e.g. `steam.exe`.
fn catalog_index_for(exe_name: &str) -> Option<usize> {
    CATALOG.iter().position(|known| {
        known.locations.iter().any(|l| Path::new(l).file_name().is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case(exe_name)))
    })
}

/// Paths the registry gives for an app: App Paths for each of its programs, and app-specific keys.
fn registry_paths(known: &Known) -> Vec<String> {
    let mut values = Vec::new();
    for location in known.locations.iter().filter(|l| !l.contains('*')) {
        let Some(exe) = Path::new(location).file_name().map(|n| n.to_string_lossy().into_owned()) else { continue };
        for (machine, root) in [(true, r"SOFTWARE"), (true, r"SOFTWARE\WOW6432Node"), (false, r"SOFTWARE")] {
            let key = format!(r"{root}\Microsoft\Windows\CurrentVersion\App Paths\{exe}");
            values.extend(win::registry_string(machine, &key, None));
        }
    }
    for (key, value) in known.user_registry {
        values.extend(win::registry_string(false, key, Some(value)));
    }
    values
        .into_iter()
        .map(|v| v.trim().trim_matches('"').replace('/', "\\"))
        .filter(|v| Path::new(v).is_absolute() && Path::new(v).is_file())
        .map(|v| std::path::absolute(&v).map(|p| p.to_string_lossy().into_owned()).unwrap_or(v))
        .collect()
}

/// Replaces the leading `%Variable%` of a catalog location. `None` when it is not set.
fn expand_vars(location: &str) -> Option<String> {
    let (variable, rest) = location.strip_prefix('%')?.split_once('%')?;
    let base: PathBuf = match variable {
        "Desktop" => win::desktop_dir()?,
        // A 32-bit build would otherwise see the x86 folder here.
        "ProgramFiles" => std::env::var_os("ProgramW6432").or_else(|| std::env::var_os("ProgramFiles")).map(PathBuf::from)?,
        other => PathBuf::from(std::env::var_os(other)?),
    };
    Some(format!("{}{rest}", base.to_string_lossy().trim_end_matches('\\')))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::validate_executable;

    #[test]
    fn catalog_locations_are_valid() {
        for known in CATALOG {
            assert!(Category::ALL.contains(&known.category));
            for location in known.locations {
                let path = expand_vars(location).unwrap_or_else(|| panic!("{location} does not expand"));
                validate_executable(&path).unwrap_or_else(|e| panic!("{}: {e}", known.name));
                assert!(catalog_index_for(&Path::new(location).file_name().unwrap().to_string_lossy()).is_some());
            }
        }
    }

    #[test]
    fn expands_variables() {
        let program_files = std::env::var("ProgramW6432").unwrap();
        assert_eq!(expand_vars(r"%ProgramFiles%\A\b.exe").unwrap(), format!(r"{program_files}\A\b.exe"));
        assert!(expand_vars(r"%NoSuchVariableForAppBlocker%\b.exe").is_none());
        assert!(expand_vars(r"C:\b.exe").is_none());
    }

    #[test]
    fn already_added_counts_patterns_and_copies() {
        let list = vec![r"C:\Apps\Chat\app-*\Chat.exe".to_string(), r"C:\Games\Steam\steam.exe".to_string()];
        assert!(covers(&list, r"c:\apps\chat\app-*\chat.exe"), "same pattern");
        assert!(covers(&list, r"C:\Apps\Chat\app-3\Chat.exe"), "concrete copy of a pattern");
        assert!(covers(&list, r"C:\Games\Steam\Steam.exe"), "same file");
        assert!(!covers(&list, r"C:\Apps\Other\app-*\Chat.exe"), "different pattern");
        assert!(!covers(&list, r"D:\Steam\steam.exe"), "another copy");
    }

    #[test]
    fn scan_does_not_fail() {
        // What is found depends on this computer; every result must be a valid, unique entry.
        let found = scan(&[]);
        for f in &found {
            validate_executable(&f.path).unwrap();
            assert!(!f.already_added);
            assert_eq!(found.iter().filter(|g| paths::same_path(&g.path, &f.path)).count(), 1, "{}", f.path);
        }
    }
}
