# Wallpaper Engine Controller

A utility to control Wallpaper Engine based on desktop visibility using libvisdesk.

## Features

- Monitors desktop visibility using libvisdesk
- Pauses/resumes Wallpaper Engine based on visibility thresholds
- Supports both global and per-monitor modes
- Can use either 32-bit or 64-bit Wallpaper Engine executable

## Installation

```
cargo install --path .
```

## Usage

```
wallpaper-controller [OPTIONS]
```

### Options

```
  -m, --monitors <MONITORS>
          Monitors to watch (comma-separated IDs, or "all" for all monitors) [default: all]

  -t, --threshold <THRESHOLD>
          Visibility threshold percentage (0-100) to trigger pause/resume [default: 50]

  -m, --mode <MODE>
          Operation mode (global or per-monitor) [default: global]
          [possible values: global, per-monitor]

      --monitor-thresholds <MONITOR_THRESHOLDS>
          Per-monitor thresholds (format: "monitor_id:threshold,...")

  -u, --update-rate <UPDATE_RATE>
          Update frequency in milliseconds [default: 500]

      --wallpaper-path <WALLPAPER_PATH>
          Path to Wallpaper Engine executable [default: "C:\Program Files (x86)\Steam\steamapps\common\wallpaper_engine"]

      --bit64
          Use 64-bit version of Wallpaper Engine (wallpaper64.exe), otherwise use 32-bit (wallpaper32.exe)

  -h, --help
          Print help

  -V, --version
          Print version
```

## Examples

### Basic usage with default settings

```
wallpaper-controller
```

This will monitor all monitors in global mode with a 50% visibility threshold and 500ms update rate, using the 32-bit Wallpaper Engine executable.

### Using the 64-bit Wallpaper Engine executable

```
wallpaper-controller --bit64
```

This will use the 64-bit version of Wallpaper Engine (wallpaper64.exe) instead of the default 32-bit version.

### Specify monitors to watch

```
wallpaper-controller --monitors 1,2
```

This will only monitor monitors with IDs 1 and 2.

### Per-monitor mode with custom thresholds

```
wallpaper-controller --mode per-monitor --monitor-thresholds "1:60,2:40" --bit64
```

This will use per-monitor mode with a 60% threshold for monitor 1 and 40% for monitor 2, using the 64-bit Wallpaper Engine executable.

### Custom update rate

```
wallpaper-controller --update-rate 1000
```

This will check visibility every 1000ms (1 second) instead of the default 500ms.

### Custom Wallpaper Engine path

```
wallpaper-controller --wallpaper-path "D:\Steam\steamapps\common\wallpaper_engine" --bit64
```

This specifies a custom path to the Wallpaper Engine executable and uses the 64-bit version.