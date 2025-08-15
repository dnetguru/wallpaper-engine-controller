# Wallpaper Controller

A utility to control Wallpaper Engine based on desktop visibility. This tool monitors the visibility of your desktop and automatically pauses and resumes Wallpaper Engine to save system resources when the desktop is not visible.

## Features

- **Automatic Control:** Pauses Wallpaper Engine when the desktop is obscured and resumes it when it becomes visible again.
- **Resource Saving:** Helps to save CPU and GPU resources by pausing Wallpaper Engine when it's not needed.
- **Customizable Threshold:** Set a custom visibility threshold to control when Wallpaper Engine should be paused.
- **Per-Monitor Mode:** Option to monitor visibility on a per-monitor basis.
- **Flexible Configuration:** Supports both 32-bit and 64-bit versions of Wallpaper Engine.
- **Windows Service:** Can be installed as a Windows service to run automatically in the background.
- **List Monitors:** A command to list all connected monitors and their identifiers.

## Installation

You can install `wallpaper-controller` in a couple of ways:

### Using a pre-compiled binary

1.  Download the latest release from the project's releases page.
2.  Place the executable in a directory of your choice.
3.  You can then run the application from the command line, or use the `--install` option to copy it to a specific location. This command requires administrator privileges.

    ```shell
    wallpaper-controller.exe --install "C:\Program Files\WallpaperController"
    ```

### Building from source

See the "Building from Source" section below.

## Usage

The basic command to run the application is:

```shell
wallpaper-controller [OPTIONS]
```

### Options

| Flag | Name | Description | Default |
| --- | --- | --- | --- |
| `-m`, `--monitors` | `<MONITORS>` | Monitors to watch. Use numbers from Display Settings, or use `-L` to list monitors (comma-separated, or "all"). | `all` |
| `-t`, `--threshold` | `<THRESHOLD>` | Minimum visibility threshold percentage (0-100) to pause Wallpaper Engine. | `20` |
| `-p`, `--per-monitor`| | Track visibility for each monitor separately. | `false` |
| `-u`, `--update-rate`| `<UPDATE_RATE>` | Maximum update frequency in milliseconds. | `1000` |
| | `--wallpaper-engine-path` | `<PATH>` | Path to Wallpaper Engine executable. | `C:\Program Files (x86)\Steam\steamapps\common\wallpaper_engine` |
| | `--64bit` | | Use the 64-bit version of Wallpaper Engine (`wallpaper64.exe`). | `false` (uses 32-bit) |
| `-L`, `--list-monitors` | | List all available monitors and their IDs, then exit. | `false` |
| | `--install` | `<PATH>` | Install the executable to the specified path and exit. Requires administrator privileges. | |
| | `--add-startup-service` | | Add a Windows service to run the program. Requires administrator privileges. | |
| | `--disable-sentry`| | Disable Sentry error reporting. | `false` |
| | `--sentry-dsn` | `<DSN>` | Override the default Sentry DSN. | |

### Examples

**Run with default settings:**
```shell
wallpaper-controller
```

**Use the 64-bit version of Wallpaper Engine:**
```shell
wallpaper-controller --64bit
```

**Monitor specific displays (e.g., monitors 1 and 2):**
```shell
wallpaper-controller --monitors 1,2
```

**Use per-monitor mode with a 10% visibility threshold:**
```shell
wallpaper-controller --per-monitor --threshold 10
```

## Running as a Service

You can set up `wallpaper-controller` to run automatically as a Windows service. This is useful if you want the application to start with Windows and run in the background.

**Important:** This feature requires that you have Wallpaper Engine's own service enabled. The `wallpaper-controller` service has a dependency on the `Wallpaper Engine Service`.

To install the service, run the following command with administrator privileges:

```shell
wallpaper-controller.exe --add-startup-service [any other flags you want to use]
```

For example, if you want to run the service with the 64-bit version of Wallpaper Engine and a 30% threshold, you would use:

```shell
wallpaper-controller.exe --add-startup-service --64bit --threshold 30
```

The service will be named **Wallpaper Controller Service**. Any arguments you provide with `--add-startup-service` will be saved and used when the service starts.

To update the service's arguments, simply run the command again with the new arguments. The existing service will be updated with the new configuration.

## Building from Source

To build `wallpaper-controller` from source, you will need to have the Rust toolchain installed.

1.  **Clone the repository:**
    ```shell
    git clone <repository-url>
    cd wallpaper-controller
    ```

2.  **Build the project:**
    ```shell
    cargo build --release
    ```
    The executable will be located in the `target/release` directory.

### Cross-compilation (from Linux to Windows)
If you are on a Linux system, you can cross-compile for Windows:
```shell
rustup target add x86_64-pc-windows-gnu
sudo apt install -y gcc-mingw-w64-x86-64 g++-mingw-w64-x86-64
cargo build --target x86_64-pc-windows-gnu --release
```
