# Wallpaper Engine Controller

A utility to control Wallpaper Engine based on desktop visibility using libvisdesk.

## Features

- Monitors desktop visibility using [libvisdesk](https://github.com/dnetguru/libvisdesk/)
- Does not poll and includes a parameter to throttle recalculations
- Pauses/resumes Wallpaper Engine based on visibility thresholds
- Supports both global and per-monitor modes
- Can use either 32-bit or 64-bit Wallpaper Engine executable
- Automatically resumes all wallpapers upon exit

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
          Minimum visibility threshold percentage (0-100) to pause the wallpaper engine [default: 20]
          
  -p, --per-monitor
          Per-monitor mode - track visibility for each monitor separately
          
  -u, --update-rate <UPDATE_RATE>
          Maximum update frequency in milliseconds [default: 1000]
          
      --wallpaper-engine-path <WALLPAPER_ENGINE_PATH>
          Path to Wallpaper Engine executable [default: "C:\\Program Files (x86)\\Steam\\steamapps\\common\\wallpaper_engine"]
          
      --64bit
          Use the 64-bit version of Wallpaper Engine (wallpaper64.exe), otherwise use 32-bit (wallpaper32.exe)
          
  -L, --list-monitors
          List all available monitors and their IDs, then exit
          
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
wallpaper-controller --monitors 0,1
```

This will only monitor monitors with IDs 0 and 1.
1
### Per-monitor mode

```
wallpaper-controller --per-monitor --threshold 10 --bit64
```

This will use per-monitor mode with a 10% threshold for all monitors, using the 64-bit Wallpaper Engine executable.

### Custom update rate

```
wallpaper-controller --update-rate 1000
```

This will check visibility AT MOST every 1000ms (1 second) instead of the default 1000ms.

### Custom Wallpaper Engine path

```
wallpaper-controller --wallpaper-path "D:\Steam\steamapps\common\wallpaper_engine" --bit64
```

This specifies a custom path to the Wallpaper Engine executable and uses the 64-bit version.

### List available monitors

```
wallpaper-controller --list-monitors
```

This will display information about all available monitors including their IDs, visible areas, and current visibility percentages. Use the displayed monitor IDs with the `--monitors` option to specify which monitors to watch.