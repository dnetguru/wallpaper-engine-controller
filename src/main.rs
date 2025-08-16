#![windows_subsystem = "windows"]

mod cli;
mod monitor;
mod wallpaper;
mod installer;

use std::hash::{DefaultHasher, Hash, Hasher};
use clap::Parser;
use tokio::signal;
use tracing::{info, error};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use sentry::integrations::tracing::EventFilter;
use sentry::ClientInitGuard;
use windows::Win32::System::Console::AllocConsole;
use windows::Win32::System::Console::AttachConsole;
use single_instance::SingleInstance;
use windows_elevate::check_elevated;

use cli::{Cli, parse_monitor_indices};
use installer::handle_installation;
use monitor::VisibilityMonitor;
use wallpaper::WallpaperController;
use crate::installer::exit_blocking;

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
    let any_instance_mutex = SingleInstance::new("Global\\WallpaperController_Any").unwrap();

    if !instance_mutex.is_single() {
        if !in_silent_mode {
            eprintln!("Another instance with the same arguments is already running.");
            drop(instance_mutex);
            drop(any_instance_mutex);
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
        .with(tracing_subscriber::fmt::layer().with_ansi(ansi_colors))
        .with(
            sentry::integrations::tracing::layer().event_filter(|md|
                match *md.level() {
                    tracing::Level::ERROR => EventFilter::Event,
                    tracing::Level::TRACE | tracing::Level::DEBUG => EventFilter::Ignore,
                    _ => EventFilter::Log,
                })
        )
        .init();

    if cli.install.is_some() || cli.add_startup_service {
        if any_instance_mutex.is_single() {
            drop(instance_mutex);
            drop(any_instance_mutex);

            handle_installation(&cli);
        } else {
            error!("There are other instances of WallpaperController running. Please close them before attempting a reinstall.");
            exit_blocking(5);
        }
    }

    // Check if we should list monitors
    if cli.list_monitors {
        print_monitor_list();
        exit_blocking(0);
    }

    // Parse monitor IDs
    let monitor_indices = parse_monitor_indices(&cli.monitors);

    // Create the wallpaper controller with the 64-bit flag
    let controller = WallpaperController::new(cli.wallpaper_engine_path, cli.bit64);

    // Create and start visibility monitoring
    let mut monitor = VisibilityMonitor::new(
        controller,
        cli.per_monitor,
        cli.threshold,
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