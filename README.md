# Windows App Blocker

Closes the Windows apps you choose during the hours you choose. No install of anything else is needed: it is a single `.exe` for Windows 10 and 11.

## Getting started

1. Download **Windows App Blocker.exe** and double-click it.
2. Follow the Windows prompts.
3. Go through setup: lock choices, apps, schedule, shortcuts. Your progress is saved as you go.
4. Turn protection on from the control panel.

Afterwards, open it from the Start menu.

## Choosing apps

- **Find common apps…** lists installed browsers, game stores, chat and music apps (Chrome, Steam, Discord, Spotify, …) so you can tick the ones to block.
- **Add .exe…** picks any other program.

## Schedule

Draw blocks on the 24-hour timeline: double-click to add, drag to move or resize, click to type exact times, **Delete** to remove. Use the same hours every day, weekdays and weekends, or each day separately. A block that ends before it starts (like 23:00–07:00) runs overnight.

## Lock choices

During setup, choose which settings you can change later. Review these choices carefully; some cannot be changed afterwards. You can review them in **Settings**.

## Updates keep your settings

To update, download the new exe, open it and select **Update installed app**. Your apps, schedule, lock choices and preferences are kept when you update or reinstall.

> Apps are force-closed, so save your work before a block starts.

## Building from source

Install Rust from [rustup.rs](https://rustup.rs), then run:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\build.ps1 -Check   # test, lint and build
```

This recreates `Windows App Blocker.exe` in the main folder.

## License

[MIT](LICENSE)
