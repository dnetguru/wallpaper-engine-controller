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

```shell
cargo install --path .
```

## Usage

```shell
wallpaper-controller [OPTIONS]
```

### Options

```sh
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

```shell
wallpaper-controller
```

This will monitor all monitors in global mode with a 50% visibility threshold and 500ms update rate, using the 32-bit Wallpaper Engine executable.

### Using the 64-bit Wallpaper Engine executable

```shell
wallpaper-controller --bit64
```

This will use the 64-bit version of Wallpaper Engine (wallpaper64.exe) instead of the default 32-bit version.

### Specify monitors to watch

```shell
wallpaper-controller --monitors 0,1
```

This will only monitor monitors with IDs 0 and 1.

### Per-monitor mode

```shell
wallpaper-controller --per-monitor --threshold 10 --bit64
```

This will use per-monitor mode with a 10% threshold for all monitors, using the 64-bit Wallpaper Engine executable.

### Custom update rate

```shell
wallpaper-controller --update-rate 1000
```

This will check visibility AT MOST every 1000ms (1 second) instead of the default 1000ms.

### Custom Wallpaper Engine path

```shell
wallpaper-controller --wallpaper-path "D:\Steam\steamapps\common\wallpaper_engine" --bit64
```

This specifies a custom path to the Wallpaper Engine executable and uses the 64-bit version.

### List available monitors

```shell
wallpaper-controller --list-monitors
```

This will display information about all available monitors including their IDs, visible areas, and current visibility percentages. Use the displayed monitor IDs with the `--monitors` option to specify which monitors to watch.

### Cross compilation on Linux
```shell
rustup target add x86_64-pc-windows-gnu
sudo apt install -y gcc-mingw-w64-x86-64 g++-mingw-w64-x86-64
cargo build --target x86_64-pc-windows-gnu
```
