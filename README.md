# Windows App Blocker

Closes the Windows apps you choose during the hours you choose. No install of anything else is needed: it is a single `.exe` for Windows 10 and 11.

## Getting started

1. Download **Windows App Blocker.exe** and double-click it.
2. Approve the administrator prompt.
3. Go through setup: lock choices, apps, schedule, shortcuts. Your progress is saved as you go.
4. Turn protection on from the control panel.

Afterwards, open it from the Start menu. Closing the window does not stop protection.

## Choosing apps

- **Find common apps…** lists installed browsers, game stores, chat and music apps (Chrome, Steam, Discord, Spotify, …) so you can tick the ones to block.
- **Add .exe…** picks any other program.

Only those exact programs are closed, and only for your Windows account. Apps that install each update into a new folder, like Discord, are added with a `*` (for example `…\Discord\app-*\Discord.exe`) so updates don't undo the block.

## Schedule

Draw blocks on the 24-hour timeline: double-click to add, drag to move or resize, click to type exact times, **Delete** to remove. Use the same hours every day, weekdays and weekends, or each day separately. A block that ends before it starts (like 23:00–07:00) runs overnight.

## Lock choices

Setup asks whether you can later change the hours, turn protection off, remove apps, uninstall, and change these lock choices. Each is **Yes — anytime**, **No — never** or **Only during allowed hours** (while the apps aren't blocked). You can see them in **Settings**, and edit them there if you allowed it.

Turning protection on and adding apps are always allowed.

## Updates keep your settings

Your apps, schedule, lock choices and preferences are saved in `C:\ProgramData\WindowsAppBlocker\config.json`, separate from the program. To update, download the new exe, open it and select **Update installed app**. Uninstalling also keeps this file, so a reinstall picks up where you left off. To start completely fresh, delete that folder after uninstalling.

> Apps are force-closed, so save your work before a block starts. This is a self-control aid, not security software: a Windows administrator can still get around it.

## Building from source

Install Rust from [rustup.rs](https://rustup.rs), then run:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\build.ps1 -Check   # test, lint and build
```

This recreates `Windows App Blocker.exe` in the main folder.

## License

[MIT](LICENSE)
