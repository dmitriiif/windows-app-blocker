# Windows App Blocker

Windows App Blocker is a small Windows 11 utility that closes apps you choose during the hours you choose.

## Start here

1. Download and extract the project folder.
2. Double-click **Windows App Blocker.exe** in the main folder.
3. Approve the Windows administrator prompt.
4. Complete the setup:
   1. **Lock choices:** decide which controls stay available later.
   2. **Apps:** pick the apps to block. You can skip this and add apps later.
   3. **Schedule:** draw your blocking periods on the 24-hour timeline. You can skip this and keep the default hours, unless you chose never to change hours later.
   4. **Finish:** choose Start menu and desktop shortcuts, then select **Install and finish setup**.
5. Turn protection on from the control panel. It needs at least one app.

Setup saves your progress as you go, so closing the window before finishing loses nothing.

After that, open **Windows App Blocker** from the Start menu. Closing the window does not stop protection.

Protection starts off, so setup cannot unexpectedly close an app.

## The timeline

- **Modes:** choose the same hours every day, separate weekday and weekend hours, or individual hours for each of the seven days. Each mode shows one lane per day or group of days.
- **Adding blocks:** double-click empty space on a lane, or use **Add block**. A lane can have as many blocks as you want, but blocks can't overlap.
- **Moving and resizing:** drag a block to move it, or drag its edges to resize it. Times snap to 5 minutes; hold **Shift** for 1-minute precision.
- **Exact times:** click a block, or its row under the timeline, to type exact `HH:MM` times.
- **Removing blocks:** press **Delete** or use **×**.
- **Copying:** **Copy to…** copies one lane's blocks to other days.
- **Overnight blocks:** a block whose end is earlier than its start (for example 23:00–07:00) runs past midnight. It shows an arrow at the right edge, and its carry-over appears hatched at the start of the next day's lane, where its end can be dragged. An overnight block finishes using the schedule of the day it started on.
- **Turning a lane off:** the switch next to a lane stops blocks starting on that day. An overnight block from the previous day still finishes.
- **Time boundaries:** the start time is included and the end time is not. End at `24:00` to block until midnight.
- **Now marker:** the red line shows the current time, and a red dot marks today's lane.

## Lock choices

Setup asks how four controls behave later:
- changing the blocking hours
- turning protection off
- removing apps from the block list
- uninstalling

Each can be **Yes — anytime**, **No — never**, or **Only during allowed hours**. "Allowed hours" are times when the schedule lets the blocked apps run.

How locks behave:
- Turning protection on is always allowed, whatever the turn-off choice.
- Adding apps is always allowed, whatever the removal choice.
- Locked controls are disabled in the control panel.
- Choosing **No — never** asks you to confirm.

## Reinstalling keeps your settings

Uninstalling removes the program, the background task and the shortcuts. **Your block list, schedule and lock choices stay in `C:\ProgramData\WindowsAppBlocker`.**

When you install again, setup starts from those settings. The lock choices are kept exactly as they were and can't be changed during reinstall. The same rules apply while you reinstall: if changing hours or removing apps is locked right now, reinstalling doesn't unlock it.

Installing over the previous (PowerShell-based) version upgrades it the same way: your apps, hours and lock choices carry over.

## How it works

The background monitor is the same program, started by Windows Task Scheduler as SYSTEM (the task is named `WindowsAppBlocker`).

During a blocking period, it checks running processes every two seconds. It force-closes any process whose full executable path exactly matches an entry in your list. It only closes processes owned by the Windows account that installed the blocker.

Matching the complete path matters: selecting `C:\Apps\Example.exe` will not block another `Example.exe` elsewhere.

| Location | Contents | On uninstall |
| --- | --- | --- |
| `C:\Program Files\Windows App Blocker` | The program | Removed |
| `C:\ProgramData\WindowsAppBlocker` | `config.json` and `WindowsAppBlocker.log` | Kept |

> Apps are force-closed. Save your work before a blocking period begins. Windows App Blocker is a self-control aid, not security software; a Windows administrator can still bypass it outside the app.

## Uninstall

If uninstalling was allowed during setup, open Windows App Blocker and select **Uninstall**. For **Only during allowed hours**, the button becomes available outside blocking periods.

To also forget your settings, delete `C:\ProgramData\WindowsAppBlocker` as an administrator after uninstalling.

## Building

The app is written in Rust.
1. Install Rust from [rustup.rs](https://rustup.rs). Either the MSVC or GNU toolchain works.
2. Run:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\build.ps1          # build
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\build.ps1 -Check   # test, lint, then build
```

This recreates `Windows App Blocker.exe` in the main folder. Tests don't close programs or change Task Scheduler.

To try the monitor without closing anything, turn protection off (only one monitor runs at a time), then run this from an elevated prompt:

```powershell
& 'C:\Program Files\Windows App Blocker\Windows App Blocker.exe' --monitor --once --dry-run
```

It logs what it would close to the log file.

## License

Windows App Blocker is available under the [MIT License](LICENSE).
