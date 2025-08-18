# Wallpaper Engine Controller

A lightweight utility that automatically pauses and resumes Wallpaper Engine based on percentage desktop visibility, using an efficient, event-driven library.\
The goal is to save CPU/GPU resources when your desktop is obscured (e.g., by fullscreen apps or windows).

## Why Use This?

Wallpaper Engine has basic occlusion detection (pausing for maximized windows on a monitor), but it doesn't handle advanced scenarios like multiple side-by-side windows obscuring the desktop.\
This tool enables using more resource-intensive wallpapers by pausing rendering when they're not visible as well as allowing the user to specify a threshold for what percentage of the dekstop needs to be visible for the rendering to continue.
\
It is recommended to specify your main monitor with `-m` (e.g., `-m 1`); if its visibility drops below the threshold, all wallpapers pause.\
The code supports per-monitor pause/resume, which can be enabled if Wallpaper Engine adds this feature in the future.\
\
**Note:** Wallpaper Engine currently [only supports](https://help.wallpaperengine.io/en/functionality/cli.html#pause) pausing/resuming all monitors at once—no per-monitor control via CLI.

## Features

- **Event-Driven Monitoring**: Uses libvisdesk for real-time desktop visibility tracking—no wasteful polling.
- **Flexible Controls**: Global or per-monitor modes (note: per-monitor pausing not yet supported by Wallpaper Engine).
- **Customizable Thresholds**: Pause when visibility drops below a set percentage (0-100).
- **Throttle Updates**: Configurable max frequency for visibility checks (default: 1000ms).
- **32/64-Bit Support**: Compatible with both Wallpaper Engine versions.
- **Graceful Shutdown**: Auto-resumes wallpapers on exit.
- **Service Mode**: Run as a Windows background service for always-on operation with automatic installation.

## Prerequisites

- Windows (tested on Windows 11).
- Wallpaper Engine installed and running (as a service for background mode).
- Rust (for building from source; see below).

## Installation

### From Releases (Recommended)

Download the latest pre-built binary from [Releases](https://github.com/dnetguru/wallpaper-engine-controller/releases) and run `wallpaper-controller.exe`.

### Running as a Windows Service

For automatic startup:\

1. Install to a permanent path and create the service in one command:  
   ```powershell  
   wallpaper-controller --install "$HOME\.wallpaper-controller\wallpaper-controller.exe" --add-startup-service -m all -t 10  
   ```  
   - This copies the exe to the specified path and sets up the service to run `wallpaper-controller.exe -m all -t 10` on Windows startup.  
   - Flags (except `--install` and `--add-startup-service`) are captured; the service always runs with them.  
   - **Note:** `--install` just copies the file. You can manually copy the file if preferred. Manually delete the file if needed.

2. To change flags: Stop all running instances of `wallpaper-controller.exe`, then rerun `--add-startup-service` with new flags (this deletes and recreates the service; no `--install` needed).

3. Manage the service via the Services app (search "services.msc") or `sc` commands (e.g., `sc start WallpaperControllerService`; delete with `sc delete WallpaperControllerService`).

4. **Note:** Requires Wallpaper Engine running as a service.  

**Additional Note:** These flags are provided for convenience. You can run the tool without installing it as a service, e.g., via scheduled tasks or manually.

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

## Quick Start

```shell  
wallpaper-controller -m all -t 10  
```  
- Monitors all displays; pauses if <10% visible.

List monitors first:  
```shell  
wallpaper-controller --list-monitors  
```  

#### Custom threshold and update rate:
This would pause Wallpaper Engine if less than 15% of your desktop is visible across all monitors. It does not start/stop wallpaper engine more frequently than every 0.5 seconds.   
```shell  
wallpaper-controller --threshold 15 --update-rate 500  
```
#### Specific monitors, 64-bit:
This would pause Wallpaper Engine if less than 20% of your desktop is visible across monitors numbered 1 and 3. All other monitors are ignored.\
The monitor numbers can be found by running the program with `-L` command and they should match the numbers shown in Windows' Display Settings.
```shell  
wallpaper-controller -m 1,3 
```

#### Custom path:
You can specify a custom path to Wallppaper Engine installation (if you are not using the default).
```shell  
wallpaper-controller --wallpaper-engine-path "D:\Games\WallpaperEngine" --64bit  
```

## CLI Options

```sh  
Usage: wallpaper-controller.exe [OPTIONS]  

Options:  
  -m, --monitors <MONITORS>  
          Monitors to watch, use numbers shown in Display Settings or use -L to list monitors (comma-separated, or "all" for all monitors) [default: all]  
  -t, --threshold <THRESHOLD>  
          Minimum visibility threshold percentage (0-100) to pause the wallpaper engine [default: 20]  
  -p, --per-monitor  
          Per-monitor mode - track visibility for each monitor separately (THIS IS NOT SUPPORTED BY WALLPAPER ENGINE, YET)  
  -u, --update-rate <UPDATE_RATE>  
          Maximum update frequency in milliseconds [default: 1000]  
      --wallpaper-engine-path <WALLPAPER_ENGINE_PATH>  
          Path to Wallpaper Engine executable [default: "C:\\Program Files (x86)\\Steam\\steamapps\\common\\wallpaper_engine"]  
      --64bit  
          Use the 64-bit version of Wallpaper Engine (wallpaper64.exe), otherwise use 32-bit (wallpaper32.exe)  
  -L, --list-monitors  
          List all available monitors and their IDs, then exit  
      --disable-sentry  
          Disable Sentry error reporting  
      --sentry-dsn <SENTRY_DSN>  
          Override the default Sentry error reporting DSN (for debugging purposes) [default: https://c6caa06487e9769daccfbedcd8de6324@o504783.ingest.us.sentry.io/4509839881076736]  
      --install <INSTALL>  
          Copy the assembly to the specified path and exit (must end with .exe)  
      --add-startup-service  
          Add a Windows service to run this program with the specified flags and exit  
  -h, --help  
          Print help  
  -V, --version  
          Print version  
```

### Silent Mode
It is possible to launch this application with `-silent` which causes it completely run in the foreground, not showing a console window.\
This is the default behavior when installed as a service.


## Contributing

Pull requests are welcome! See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## License

MIT – see [LICENSE](LICENSE).
