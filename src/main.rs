#![windows_subsystem = "windows"]

mod cli;
mod monitor;
mod wallpaper;
mod install;

use std::{env, thread};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::process::Command;
use std::time::Duration;
use clap::Parser;
use tokio::signal;
use tracing::{info, error, warn};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use sentry::integrations::tracing::EventFilter;
use sentry::ClientInitGuard;
use windows::Win32::System::Console::AllocConsole;
use windows::Win32::System::Console::{AttachConsole};
use single_instance::SingleInstance;
use windows_elevate::{check_elevated, elevate};
use anyhow::{Result, anyhow};

use cli::{Cli, parse_monitor_indices};
use install::handle_installation;
use monitor::VisibilityMonitor;
use wallpaper::WallpaperController;
use crate::install::exit_blocking;
use crate::install::tui::run_install_tui_and_relaunch;

#[tokio::main(worker_threads = 2)]
async fn main() {
    let raw_args: Vec<String> = std::env::args().collect();
    let in_silent_mode = raw_args.iter().any(|a| a == "-silent");

    let mut filtered_args: Vec<String> = raw_args
        .clone()
        .into_iter()
        .filter(|a| !["-safe", "-silent", "-service"].contains(&a.as_str()))
        .collect();

    let ansi_colors =
        if in_silent_mode {
            false
        } else if check_elevated().unwrap_or(false) || unsafe { AttachConsole(u32::MAX) }.is_err() {
            unsafe { AllocConsole() }.ok();
            false
        } else {
            println!();
            true
        };

    let mut cli = Cli::parse_from(&filtered_args);

    // Sort the filtered args for unique key
    filtered_args.sort();

    // Create a unique mutex name based on sorted args
    let mut hasher = DefaultHasher::new();
    filtered_args[1..].join("|").hash(&mut hasher);
    let instance_mutex = SingleInstance::new(&format!("Global\\WallpaperController_{}", hasher.finish())).unwrap();

    if !instance_mutex.is_single() {
        if !in_silent_mode {
            eprintln!("Another instance with the same arguments is already running.");
            drop(instance_mutex);
            exit_blocking(5);
        }
        return;
    }

    let _guard: ClientInitGuard;
    if !cli.disable_sentry {
        _guard = sentry::init((cli.sentry_dsn.take(), sentry::ClientOptions {
            release: sentry::release_name!(),
            enable_logs: true,
            ..Default::default()
        }));
    }

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"))
        .add_directive("hyper=warn".parse().unwrap())
        .add_directive("hyper_util=warn".parse().unwrap())
        .add_directive("reqwest=warn".parse().unwrap())
        .add_directive("h2=warn".parse().unwrap())
        .add_directive("http=warn".parse().unwrap())
        .add_directive("sentry=warn".parse().unwrap())
        .add_directive("sentry_core=warn".parse().unwrap())
        .add_directive("sentry_tracing=warn".parse().unwrap());

    tracing_subscriber::registry()
        .with(filter, )
        .with(tracing_subscriber::fmt::layer().with_ansi(ansi_colors).without_time())
        .with(
            sentry::integrations::tracing::layer().event_filter(|md|
                match *md.level() {
                    tracing::Level::ERROR => EventFilter::Event,
                    tracing::Level::TRACE | tracing::Level::DEBUG => EventFilter::Ignore,
                    _ => EventFilter::Log,
                })
        )
        .init();

    // Check if the user asked to list monitors
    if cli.list_monitors {
        print_monitor_list();
        exit_blocking(0);
    }

    if (raw_args.len() <= 1) || cli.install_tui {
        elevate_and_kill_others(instance_mutex);
        if let Err(e) = run_install_tui_and_relaunch(cli) {
            error!("Installation aborted: {}", e);
        }

        std::process::exit(0);
    }

    if cli.install_dir.is_some() || cli.add_startup_service || cli.add_startup_task {
        if cli.add_startup_service && cli.add_startup_task {
            error!("Cannot use both --add-startup-service and --add-startup-task");
            exit_blocking(8);
        }

        elevate_and_kill_others(instance_mutex);
        handle_installation(&cli);
        return;
    }

    // Parse monitor IDs
    let monitor_indices = parse_monitor_indices(&cli.monitors);

    // Create the wallpaper controller with the 64-bit flag
    let controller = WallpaperController::new(cli.wallpaper_engine_path, cli.bit64);

    // Create and start visibility monitoring
    let mut monitor = VisibilityMonitor::new(
        controller,
        cli.per_monitor,
        cli.threshold.unwrap_or(20),
        monitor_indices,
    );

    if monitor.start_monitoring(cli.update_rate).await {
        info!("Started monitoring desktop visibility");

        if let Err(err) = signal::ctrl_c().await {
            error!("Unable to listen for shutdown signal: {}", err);
        } else {
            info!("Ctrl+C received");
        }

        info!("Stopping monitoring task...");
        if monitor.stop_monitoring().await {
            info!("Stopped monitoring task");
        } else {
            error!("Failed to stop monitoring task");
        }
    } else {
        error!("Failed to start monitoring task");
    }
}

fn elevate_and_kill_others(instance_mutex: SingleInstance) {
    if !check_elevated().unwrap_or(false) {
        info!("Requesting administrator privileges...");
        info!("Process will continue in a new window");
        drop(instance_mutex);

        if let Err(e) = elevate() {
            error!("Failed to elevate process: {:?}", e);
        }

        std::process::exit(0); // Exit the non-elevated process
    } else {
        kill_other_instances().ok();
    }
}

fn kill_other_instances() -> Result<()> {
    // Determine the image name of the current executable
    let this_exe = env::current_exe()?;
    let image_name = this_exe.file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| anyhow!("Failed to determine current executable name"))?
        .to_string();

    let this_pid = std::process::id();
    info!("Attempting to terminate other running instances of {}...", image_name);

    // Query tasklist for processes with the same image name in CSV for easier parsing -- somewhat hacky but works
    let output = Command::new("tasklist")
        .args(["/FI", &format!("IMAGENAME eq {}", image_name), "/FO", "CSV"])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        warn!("tasklist failed while searching for other instances: {}", stderr);
        return Ok(());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Collect target PIDs (excluding self)
    let mut target_pids: Vec<u32> = Vec::new();
    for (i, line) in stdout.lines().enumerate() {
        if i == 0 { continue; } // skip header
        let trimmed = line.trim();
        if trimmed.is_empty() { continue; }
        // CSV fields quoted, expect: "Image Name","PID","Session Name","Session#","Mem Usage"
        let parts: Vec<String> = trimmed.split(',')
            .map(|s| s.trim().trim_matches('"').to_string())
            .collect();
        if parts.len() < 2 { continue; }
        if let Ok(pid) = parts[1].parse::<u32>() {
            if pid != this_pid { target_pids.push(pid); }
        }
    }

    if target_pids.is_empty() {
        return Ok(());
    }

    // First, try a graceful termination using taskkill without /F (no console tricks)
    for pid in &target_pids {
        let res = Command::new("taskkill")
            .args(["/PID", &pid.to_string()])
            .output();
        match res {
            Ok(out) => {
                if out.status.success() {
                    info!("Requested graceful termination for PID {} ({})", pid, image_name);
                } else {
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    warn!("Graceful taskkill failed for PID {}: {}", pid, stderr);
                }
            }
            Err(e) => warn!("taskkill (graceful) failed for PID {}: {}", pid, e),
        }
    }

    // Wait 2.5 seconds to allow graceful shutdown
    thread::sleep(Duration::from_millis(2500));

    // Force-kill any remaining instances
    let output2 = Command::new("tasklist")
        .args(["/FI", &format!("IMAGENAME eq {}", image_name), "/FO", "CSV"])
        .output()?;

    if !output2.status.success() {
        let stderr = String::from_utf8_lossy(&output2.stderr);
        warn!("tasklist failed while verifying remaining instances: {}", stderr);
        return Ok(());
    }

    let stdout2 = String::from_utf8_lossy(&output2.stdout);
    let mut killed_any = false;

    for (i, line) in stdout2.lines().enumerate() {
        if i == 0 { continue; }
        let trimmed = line.trim();
        if trimmed.is_empty() { continue; }
        let parts: Vec<String> = trimmed
            .split(',')
            .map(|s| s.trim().trim_matches('"').to_string())
            .collect();
        if parts.len() < 2 { continue; }
        if let Ok(pid) = parts[1].parse::<u32>() {
            if pid == this_pid { continue; }
            if !target_pids.contains(&pid) { continue; } // only those we targeted

            let kill = Command::new("taskkill").args(["/PID", &pid.to_string(), "/F"]).output();
            match kill {
                Ok(res) => {
                    if res.status.success() {
                        info!("Force terminated process PID {} ({})", pid, image_name);
                        killed_any = true;
                    } else {
                        let stderr = String::from_utf8_lossy(&res.stderr);
                        warn!("Failed to force terminate PID {}: {}", pid, stderr);
                    }
                }
                Err(e) => warn!("taskkill failed for PID {}: {}", pid, e),
            }
        }
    }

    if killed_any {
        // Allow a brief moment for the OS to release file handles
        thread::sleep(Duration::from_millis(500));
    }

    Ok(())
}

fn print_monitor_list() {
    info!("Listing available monitors...");

    // Create a temporary instance to get monitor information
    let instance = libvisdesk::LibVisInstance::new();
    let (monitors, total_visible, total_area) = instance.get_visible_area();

    println!("\nAvailable Monitors:");
    println!("-------------------");
    println!("Total visible area: {} pixels", total_visible);
    println!("Total desktop area: {} pixels", total_area);
    // Calculate max_visible sum for proper visibility calculation
    let total_max_visible: i64 = monitors.iter().map(|m| m.max_visible).sum();
    println!("Overall visibility: {:.1}%\n", (total_visible as f64 / total_max_visible as f64 * 100.0));

    for monitor in monitors.iter() {
        let visibility_percent = if monitor.max_visible > 0 {
            monitor.current_visible as f64 / monitor.max_visible as f64 * 100.0
        } else {
            0.0
        };

        println!("Monitor number {} (as shown in Display Settings)", monitor.monitor_index);
        println!("  Total area:\t\t{} pixels", monitor.total_area);
        println!("  Maximum visible:\t{} pixels", monitor.max_visible);
        println!("  Current visible:\t{} pixels", monitor.current_visible);
        println!("  Visibility:\t\t{:.1}%", visibility_percent);
        println!("  Display number:\t{}\n", monitor.monitor_id);
    }

    println!("Use these Monitor numbers (1, 2, 3, etc.) with the --monitors option to specify which monitors to watch.");
}
