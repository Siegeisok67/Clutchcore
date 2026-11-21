<p align="center">
  <img src="assets/logo256.png" width="112" alt="Clutchcore logo" />
</p>

<h1 align="center">Clutchcore</h1>

<p align="center">
  <strong>A competitive-game tuner for Windows.</strong><br />
  Lower input latency, steadier frame pacing and cleaner network paths — with every original setting saved before anything changes.
</p>

<p align="center">
  <a href="https://github.com/Siegeisok67/Clutchcore/releases/latest"><strong>Download the latest release</strong></a>
</p>

---

## Download

Grab **`Clutchcore-Setup-1.0.4.exe`** from the [latest release](https://github.com/Siegeisok67/Clutchcore/releases/latest).

- Windows 10 or 11, 64-bit.
- Per-user install into `%LOCALAPPDATA%\Programs\Clutchcore`; no administrator rights are needed to install.
- A Start menu shortcut is created and a desktop shortcut is optional.
- The setup file is self-contained — the application is embedded inside it, so there is nothing else to download.

The app and installer are not code-signed yet, so Windows SmartScreen may show a warning on first run. Choose **More info → Run anyway**.

## What Clutchcore does

Most competitive tuning advice is a list of registry edits with no explanation and no way back. Clutchcore turns that into a small, readable application:

- **Scans the machine** — CPU, GPU and VRAM, memory, displays and refresh rates, storage, running game processes and startup load.
- **Scores competitive readiness** across input, latency, FPS and network, then suggests the highest-impact fixes first.
- **Applies reversible Windows tweaks** for mouse input, Game DVR and Game Mode, fullscreen behaviour, power plans, scheduling priority, visual effects, the TCP stack and network adapter power saving.
- **Captures every original value before writing.** Settings can be reverted one at a time, from a snapshot, or all at once.
- **Runs network route probes** to game-provider gateways and ranks them by TCP handshake time, jitter and packet loss.
- **Ships game presets and system presets**, plus fully custom profiles.
- **Works headless too**, for scripted checks and reports.

Clutchcore does **not** install drivers or services, does not read or modify game files, anti-cheat software or process memory, and has no telemetry. The only network traffic it generates is the route probe you explicitly start.

## The app

| View | What it gives you |
| --- | --- |
| **Dashboard** | Readiness score and grade, input/latency/FPS/network breakdown, hardware summary, priority fixes, live game processes with a one-click priority boost, and the core toggles. |
| **System scan** | Full hardware and display report, storage, startup entries and detected games. Exportable as a text report. |
| **Network lab** | Probe 10 routes across Riot, Battle.net, Steam, Epic, EA, Ubisoft, Call of Duty and Minecraft gateways, or test any hostname/IP with a custom probe. Ranks routes by handshake time, jitter and loss. |
| **Tweak library** | All 16 tweaks with risk level, administrator/reboot requirements, current state and the exact registry values involved. |
| **Profiles & game presets** | Eleven game presets, three system presets, and a custom profile editor. Each preset includes launch options, config file locations and an in-game checklist. |
| **Backup vault** | The first captured value for every tweak, timestamped snapshots for every change, restore actions and readable text exports. |
| **Activity log** | Apply/revert results, failures and the full history, plus the headless command reference. |

### Game presets

VALORANT · Counter-Strike 2 · Fortnite · Apex Legends · Overwatch 2 · League of Legends · Rainbow Six Siege · Rust · Call of Duty: Warzone · Minecraft · Rocket League

Each preset is a curated Windows tweak set plus game-specific launch options and settings advice. Applying a preset never edits game files.

### System presets

- **Safe Street** — low-risk, user-level changes only.
- **Full Clutch** — everything, including administrator-level scheduling and network changes.
- **Network Focus** — the TCP stack and adapter power path only.

### Included tweaks

Sixteen reversible changes across five categories:

- **Input** — mouse acceleration off.
- **Latency** — Game DVR, fullscreen optimizations, Game Mode, Xbox Game Bar, hardware-accelerated GPU scheduling, MMCSS game priority, Win32 priority separation.
- **Power / system** — high-performance power plan, power throttling off, visual effects for performance, transparency off, background app trimming.
- **Network** — Nagle/delayed-ACK latency tweaks, multimedia network throttling off, adapter power saving off.

The exact list, risk levels and requirements are shown inside the app before anything is applied.

## Safety and reversibility

- **Originals first.** Before a value is written, the existing value is stored in `%APPDATA%\Clutchcore\originals.json`.
- **A snapshot per action.** Every apply and revert creates a timestamped snapshot under `%APPDATA%\Clutchcore\snapshots\`.
- **Plain files, no lock-in.** Backups are JSON and the log is plain text. The vault can export a human-readable copy.
- **Toggle-off means restore.** Turning a tweak off writes the captured original back, not a guessed Windows default.
- **Risk is labelled.** Medium/high-risk changes ask for confirmation and explain what they affect; administrator and reboot requirements are shown up front.
- **Nothing runs by itself.** Profiles are applied only when you click Apply.

If something feels worse, open **Backup vault → Restore every original**, or restore a single snapshot.

## Building from source

Requirements: Rust (stable, MSVC toolchain) on Windows. The icon and version resource step uses `rc.exe` from the Windows SDK when it is available; the build still succeeds without it, just without the embedded file icon.

```bash
# 1. Build the application
cargo build --release

# 2. Stage the binary the installer embeds
mkdir -p dist
cp target/release/Clutchcore.exe dist/Clutchcore.exe

# 3. Build the installer
cd installer
cargo build --release
cp target/release/ClutchcoreSetup.exe ../dist/Clutchcore-Setup-1.0.4.exe
```

The installer embeds `dist/Clutchcore.exe`, compressed, at build time. Rebuild step 1–2 before step 3 whenever the app changes.

### Project layout

```
src/            application (engine, views, scanning, network, backups)
src/views/      one module per screen
installer/      standalone Rust installer / uninstaller crate
assets/         logo, ICO frames and raw icon data
tools/icogen/   logo → ICO + RGBA converter
build.rs        embeds the Windows icon and version resource
```

## Command line

The same engine works without the GUI:

```text
Clutchcore.exe --scan                 Print a system report
Clutchcore.exe --tweaks               List tweaks and their current state
Clutchcore.exe --profiles             List game and system presets
Clutchcore.exe --apply-profile <id>   Apply a preset or custom profile
Clutchcore.exe --probe                Run the network route probe
Clutchcore.exe --restore-all          Restore every captured original
Clutchcore.exe --selftest             Run the built-in checks
Clutchcore.exe --help                 Full usage
```

The installer accepts `--silent`, `--dir <path>`, `--no-desktop`, `--launch`, `--uninstall` and `--keep-data` for scripted installs and removals.

## Where your data lives

| Path | Contents |
| --- | --- |
| `%LOCALAPPDATA%\Programs\Clutchcore\` | Application and uninstaller |
| `%APPDATA%\Clutchcore\originals.json` | First captured value for every changed tweak |
| `%APPDATA%\Clutchcore\snapshots\` | Timestamped backups, one per apply or revert |
| `%APPDATA%\Clutchcore\profiles.json` | Custom profiles and the active profile |
| `%APPDATA%\Clutchcore\clutchcore.log` | Activity log |

Set `CLUTCHCORE_DATA` to use a different data folder. Uninstalling keeps profiles and backups by default; the uninstaller has an option to delete them.

## Roadmap

Planned for future releases:

- **Linux support.** A native Linux build is planned. Clutchcore is Windows-specific today because its tweaks are registry, power-plan and Windows network-stack changes.
- **More games and more per-game options.** Expanding beyond the current presets with deeper, game-specific tuning paths.
- **More optimizations, especially networking.** Deeper TCP/UDP and adapter tuning, bufferbloat-aware analysis, route comparison and jitter reduction, plus further frame-pacing and input-path work.

## Current limitations

- **Windows only.** Registry and power-plan based by design.
- **TCP probes are a proxy.** Route measurements use TCP handshakes to provider gateways, not in-game UDP. Use them to compare routes, not as an absolute in-game ping.
- **Some changes need administrator rights or a reboot.** Clutchcore labels them before applying.
- **Unsigned binaries.** SmartScreen will warn until the project is code-signed.
- **Anti-cheat is untouched.** Clutchcore never reads or writes game files, anti-cheat components or process memory.

---

<p align="center"><sub>Clutchcore 1.0.4 · built for people who want their machine consistent, not just faster on paper.</sub></p>
