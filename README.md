# Wallpaper Engine Controller

A lightweight utility that automatically pauses and resumes Wallpaper Engine based on desktop visibility, using an efficient, event-driven library. The goal is to saves CPU/GPU resources when your desktop is obscured (e.g., by fullscreen apps or windows).

## Why Use This?
While Wallpaper Engine has basic occlusion detection (pausing for maximized windows), it doesn't handle advanced scenarios like multiple side-by-side windows obscuring most of the desktop.\
This tool enables using more resource-intensive wallpapers by pausing rendering when they're not visible.\
\
Note: Wallpaper Engine currently only supports pausing/resuming all monitors at once—no per-monitor control.\
Recommend specifying your main monitor with `-m` (e.g., `-m 1`); if its visibility drops below threshold, all pause.\
The code supports per-monitor pause/resume which can be enabled if Wallpaper Engine adds it.

## Features
- **Event-Driven Monitoring**: Uses libvisdesk for real-time desktop visibility tracking—no wasteful polling.
- **Flexible Controls**: Global or per-monitor modes (note: per-monitor pausing not yet supported by Wallpaper Engine).
- **Customizable Thresholds**: Pause when visibility drops below a set percentage (0-100).
- **Throttle Updates**: Configurable max frequency for visibility checks (default: 1000ms).
- **32/64-Bit Support**: Compatible with both Wallpaper Engine versions.
- **Graceful Shutdown**: Auto-resumes wallpapers on exit or Ctrl+C.
- **Monitor Listing**: Easily identify monitor IDs with `--list-monitors`.
- **Service Mode**: Run as a Windows background service for always-on operation.
- **Sentry Integration**: Optional error reporting (disabled by default).

## Prerequisites
- Windows (tested on 11).
- Wallpaper Engine installed and running (as a service for background mode).
- Rust (for building from source; see below).

## Installation

### From Releases (Recommended)
Download the latest pre-built binary from [Releases](https://github.com/dnetguru/wallpaper-engine-controller/releases) and run `wallpaper-controller.exe`.

### Run as a Windows Service
For automatic startup:
1. Install to a permanent path and create the service in one command:
```powershell
wallpaper-controller --install "$HOME\.wallpaper-controller\wallpaper-controller.exe" --add-startup-service -m all -t 10
```
   - This copies the exe to the specified path and sets up the service to run `wallpaper-controller.exe -m all -t 10` on Windows startup.
   - Flags (except `--install` and `--add-startup-service`) are captured; the service always runs with them.
   - Note: `--install` just copies the file. You can always manually copy the file over. Manually delete the file if needed.
2. To change flags: Stop the service (via Services app or `sc stop WallpaperControllerService`), then rerun `--add-startup-service` with new flags (deletes/recreates service; no `--install` needed).
3. Manage via Services app (search "services.msc") or `sc` (e.g., `sc start WallpaperControllerService`; delete with `sc delete`).
4. Note: Requires Wallpaper Engine as a service.\
\
NOTE: These flags are there for your convenience. This does not need to run as a Windows service. You can always manually copy the exe and run it via schedueld tasks or any other way you prefer.

### Building from Source
```shell
git clone https://github.com/dnetguru/wallpaper-engine-controller.git
cd wallpaper-engine-controller
cargo build --release
```

For cross-compilation on Linux:
```shell
rustup target add x86_64-pc-windows-gnu
sudo apt install -y gcc-mingw-w64-x86-64 g++-mingw-w64-x86-64
cargo build --target x86_64-pc-windows-gnu --release
```

## Usage

### Quick Start
```shell
wallpaper-controller -m all -t 10
```
- Monitors all displays; pauses if <10% visible.

List monitors first:
```shell
wallpaper-controller --list-monitors
```
Output example:
```text
Available Monitors:
-------------------
Total visible area: 1234567 pixels
Total desktop area: 2345678 pixels
Overall visibility: 52.7%

Monitor number 1 (as shown in Display Settings)
  Total area:		2073600 pixels
  Maximum visible:	2073600 pixels
  Current visible:	1094400 pixels
  Visibility:		52.8%
  Display number:	0
```

### CLI Options
```sh
Options:
  -m, --monitors <MONITORS>
          Monitors to watch (comma-separated IDs from Display Settings, or "all") [default: all]
  -t, --threshold <THRESHOLD>
          Visibility % below which to pause (0-100) [default: 20]
  -p, --per-monitor
          Track each monitor separately (experimental; Wallpaper Engine limitation)
  -u, --update-rate <UPDATE_RATE>
          Max check frequency (ms; lower = more responsive, higher CPU) [default: 1000]
      --wallpaper-engine-path <PATH>
          Wallpaper Engine install dir [default: "C:\Program Files (x86)\Steam\steamapps\common\wallpaper_engine"]
      --64bit
          Use 64-bit executable (wallpaper64.exe)
  -L, --list-monitors
          List monitors and exit
      --disable-sentry
          Disable error reporting
      --sentry-dsn <DSN>
          Custom Sentry DSN (debug)
      --install <PATH>
          Install exe to path (and optionally service)
      --add-startup-service
          Create auto-start service with flags
  -h, --help
          Print help
  -V, --version
          Print version
```

### Examples
- Global mode, custom threshold (pauses all if total visibility <15%):
```shell
wallpaper-controller --threshold 15 --update-rate 500
```

- Specific monitors, 64-bit (specifying monitor numbers with -m causes calculations to only use desktops on those monitors; pauses all via WE):
```shell
wallpaper-controller -m 1,3 --64bit
```

- Custom path:
```shell
wallpaper-controller --wallpaper-engine-path "D:\Games\WallpaperEngine" --64bit
```

## Contributing
Pull requests welcome! See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## License
MIT – see [LICENSE](LICENSE).
