use std::path::{Path, PathBuf};
use dialoguer::{theme::ColorfulTheme, Confirm, Input, Select};
use windows_service::service_manager::{ServiceManager, ServiceManagerAccess};
use windows_service::service::ServiceAccess;
use anyhow::{anyhow, Result};
use tracing::error;
use crate::cli::Cli;
use crate::install::WALLPAPER_ENGINE_SERVICE_NAME;

fn wallpaper_engine_service_exists() -> bool {
    match ServiceManager::local_computer(None::<&std::ffi::OsStr>, ServiceManagerAccess::all()) {
        Ok(manager) => manager
            .open_service(WALLPAPER_ENGINE_SERVICE_NAME, ServiceAccess::QUERY_STATUS)
            .is_ok(),
        Err(e) => {
            error!("Failed to query services: {:?}", e);
            false
        },
    }
}

fn validate_install_dir(s: &str) -> std::result::Result<(), String> {
    fn looks_like_file_path(p: &str) -> bool {
        Path::new(p)
            .extension()
            .is_some_and(|ext| ext.to_string_lossy().eq_ignore_ascii_case("exe"))
    }

    let t = s.trim();
    if t.is_empty() { return Err("Please enter a directory path".into()); }
    if looks_like_file_path(t) { return Err("This looks like a file path (.exe). Please provide a folder".into()); }
    if t.chars().any(|c| ['<', '>', '"', '|', '?', '*'].contains(&c)) { return Err("Path contains invalid Windows characters: <>:\"|?*".into()); }
    let p = Path::new(t);
    if p.exists() && p.is_file() { return Err("Path points to a file; please provide a directory".into()); }
    Ok(())
}

fn validate_threshold(s: &str) -> std::result::Result<(), String> {
    if s.trim().is_empty() { return Err("Please enter a number between 0 and 100".into()); }
    match s.trim().parse::<u8>() {
        Ok(v) if v <= 100 => Ok(()),
        _ => Err("Threshold must be an integer between 0 and 100".into()),
    }
}

fn validate_monitors(s: &str) -> std::result::Result<(), String> {
    let t = s.trim().to_lowercase();
    if t == "all" { return Ok(()); }
    if t.is_empty() { return Err("Enter 'all' or a list like 1,2,3".into()); }
    for part in t.split(',') {
        let p = part.trim();
        if p.is_empty() { return Err("Invalid list: contains empty entries".into()); }
        match p.parse::<u32>() {
            Ok(n) if n >= 1 => (),
            _ => return Err(format!("'{}' is not a valid monitor number (must be >= 1)", p)),
        }
    }
    Ok(())
}

fn validate_update_rate(s: &str) -> std::result::Result<(), String> {
    match s.trim().parse::<u64>() {
        Ok(ms) if (100..=60000).contains(&ms) => Ok(()),
        _ => Err("Enter an integer between 100 and 60000 ms".into()),
    }
}

// fn validate_we_path(s: &str, require_64: bool) -> std::result::Result<(), String> {
//     let p = Path::new(s.trim());
//     if !p.exists() || !p.is_dir() { return Err("Path must exist and be a folder".into()); }
//     let ok = if require_64 {
//         p.join("wallpaper64.exe").exists()
//     } else {
//         p.join("wallpaper32.exe").exists() || p.join("wallpaper64.exe").exists()
//     };
//     if !ok {
//         return Err(if require_64 {
//             "Could not find wallpaper64.exe in this folder".into()
//         } else {
//             "Could not find wallpaper32.exe or wallpaper64.exe in this folder".into()
//         });
//     }
//     Ok(())
// }

pub fn run_install_tui_and_relaunch(base: Cli) -> Result<()> {
    // Run the wizard to collect all settings
    let new_cli = run_install_tui(base)?;

    // Build argv and relaunch current executable
    let exe = std::env::current_exe()?;
    let mut args: Vec<std::ffi::OsString> = Vec::new();

    if let Some(dir) = &new_cli.install_dir {
        args.push("--install-dir".into());
        args.push(dir.clone().into());
    }
    if new_cli.add_startup_service { args.push("--add-startup-service".into()); }
    if new_cli.add_startup_task { args.push("--add-startup-task".into()); }

    // runtime flags
    args.push("-m".into());
    args.push(new_cli.monitors.clone().into());

    if let Some(t) = new_cli.threshold { args.push("-t".into());args.push(t.to_string().into()); }
    if new_cli.per_monitor { args.push("-p".into()); }

    args.push("-u".into());
    args.push(new_cli.update_rate.to_string().into());

    // TODO: This won't work with wallpaperservice32.exe
    // if !new_cli.wallpaper_engine_path.is_empty() {
    //     args.push("-w".into());
    //     args.push(format!("'{}'", new_cli.wallpaper_engine_path).into());
    // }

    if new_cli.bit64 { args.push("--64bit".into()); }

    if new_cli.disable_sentry { args.push("--disable-sentry".into()); }
    if let Some(dsn) = &new_cli.sentry_dsn { args.push("--sentry-dsn".into()); args.push(dsn.clone().into()); }
    std::process::Command::new(exe).args(args).spawn()?;

    Ok(())
}

pub fn run_install_tui(mut base: Cli) -> Result<Cli> {
    let theme = ColorfulTheme::default();

    println!("\nWallpaper Controller - Interactive Installer\n");

    // Detect Wallpaper Engine Service state
    let we_service = wallpaper_engine_service_exists();

    // Explain startup modes
    println!("Startup mode choices:\n  • Windows Service: Starts early (before user logon). Requires Wallpaper Engine's 'High Priority (Run as service)'.\n    Use this if you rely on the WE service and want earliest startup.\n  • Scheduled Task: Starts at user logon with highest privileges. Works even if the WE service is disabled.\n");

    let modes: Vec<&str> = if we_service {
        vec![
            "Install as Windows Service (recommended)",
            "Install as Scheduled Task at logon",
        ]
    } else {
        println!("Note: Wallpaper Engine Service not detected.");
        println!("Either enable it in WE settings (General → Start with Windows → High Priority) and rerun, or choose a Scheduled Task.");
        vec![
            "Install as Scheduled Task at logon (recommended)",
            "I will enable Wallpaper Engine's High Priority service and retry",
        ]
    };

    let mode_idx = Select::with_theme(&theme)
        .with_prompt(if we_service { "Choose how Wallpaper Controller should run on startup" } else { "Service not detected. How would you like to proceed?" })
        .items(&modes)
        .default(0)
        .interact()?;

    if !we_service && mode_idx == 1 {
        println!("\nOpen Wallpaper Engine → Settings → General and set 'Start with Windows' to High Priority (Run as service). After enabling, run this installer again.\n");
        return Err(anyhow!("User opted to enable service and retry"));
    }

    // Derive startup choice
    let install_as_service = we_service && mode_idx == 0;
    let install_as_task = !install_as_service;

    // Install directory (validated)
    let default_dir_str = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("C:\\Users\\Public"))
        .join(".wallpaper-controller")
        .to_string_lossy().to_string();

    println!("\n• Install location: Press enter to accept the default location");
    let install_dir: String = Input::with_theme(&theme)
        .with_prompt("Install directory")
        .default(default_dir_str)
        .validate_with(|s: &String| validate_install_dir(s))
        .interact_text()?;

    // Monitors (validated)
    println!("\n• Monitors: Specify which monitors should be considered when calculating desktop visibility.\n   Specify 'all' to monitor all displays\n   Or specify a comma-separated list of display numbers as shown in Windows Display Settings (e.g., 1,2)");
    base.monitors = Input::with_theme(&theme)
        .with_prompt("Monitors to watch ('all' or e.g. '2' or '1,2')")
        .default(base.monitors.clone())
        .validate_with(|s: &String| validate_monitors(s))
        .interact_text()?;

    // Threshold (mandatory)
    println!("\n• Visibility threshold: Percentage of the desktop (across enabled monitors) that must remain visible before wallpapers are paused.");
    let th_str: String = Input::with_theme(&theme)
        .with_prompt("Visibility threshold (0–100)")
        .default("20".into())
        .validate_with(|s: &String| validate_threshold(s))
        .interact_text()?;
    base.threshold = Some(th_str.trim().parse::<u8>()?);

    // Advanced options
    println!();
    let advanced = Confirm::with_theme(&theme)
        .with_prompt("Open advanced configuration?")
        .default(false)
        .interact()?;

    if advanced {
        println!("\n• Update rate: Minimum time between visibility recalculations (in milliseconds).\n   Lower = more responsive, higher CPU and more frequent pause/resume for Wallpaper Engine.");
        let upd_str: String = Input::with_theme(&theme)
            .with_prompt("Update rate in ms (100–60000)")
            .default(base.update_rate.to_string())
            .validate_with(|s: &String| validate_update_rate(s))
            .interact_text()?;
        base.update_rate = upd_str.trim().parse::<u64>()?;

        base.bit64 = Confirm::with_theme(&theme)
            .with_prompt("• Use 64-bit Wallpaper Engine (wallpaper64.exe)?")
            .default(base.bit64)
            .interact()?;

        // TODO: See TODO note in run_install_tui_and_relaunch
        // println!("\n• Wallpaper Engine folder:");
        // base.wallpaper_engine_path = Input::with_theme(&theme)
        //     .with_prompt("Wallpaper Engine install path")
        //     .default(base.wallpaper_engine_path.clone())
        //     .validate_with(|s: &String| validate_we_path(s, base.bit64))
        //     .interact_text()?;
    }

    // Summary & confirmation
    println!(
        "\nSummary:\n  Startup: {}\n  Install dir: {}\n  Threshold: {}\n  Monitors: {}\n  Update rate: {} ms\n  WE 64-bit: {}\n  WE path: {}\n",
        if install_as_service { "Windows Service" } else { "Scheduled Task at logon" },
        install_dir,
        base.threshold.unwrap(),
        base.monitors,
        base.update_rate,
        base.bit64,
        base.wallpaper_engine_path,
    );
    let proceed = Confirm::with_theme(&theme)
        .with_prompt("Proceed with installation?")
        .default(true)
        .interact()?;
    if !proceed { return Err(anyhow!("User cancelled")); }

    // Fill internal fields consumed by the installer
    base.install_dir = Some(install_dir);
    base.add_startup_service = install_as_service;
    base.add_startup_task = install_as_task;

    Ok(base)
}
