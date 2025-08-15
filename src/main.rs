mod cli;
mod monitor;
mod wallpaper;
mod installer;

use std::ffi::OsString;
use std::time::Duration;
use clap::Parser;
use installer::handle_installation;
use sentry::ClientInitGuard;
use tokio::signal;
use tokio::sync::mpsc;
use tracing::{info, error};
use sentry::integrations::tracing::EventFilter;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use windows_service::{define_windows_service, service_dispatcher};
use windows_service::service::{ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus, ServiceType};
use windows_service::service_control_handler::{self, ServiceControlHandlerResult};
use cli::{Cli, parse_monitor_indices};
use monitor::VisibilityMonitor;
use wallpaper::WallpaperController;
use tokio::pin;


define_windows_service!(service_main_ffi, service_main);
pub const SERVICE_NAME: &str = "WallpaperControllerService";
pub const SERVICE_DISPLAY_NAME: &str = "Wallpaper Controller Service";

fn main() {
    let cli = Cli::parse();

    if cli.service {
        service_dispatcher::start(SERVICE_NAME, service_main_ffi).expect("Failed to start service dispatcher");
        return;
    }

    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(app(cli, None));
}


fn service_main(_args: Vec<OsString>) {
    let cli = Cli::parse();

    // Create shutdown channel
    let (shutdown_tx, shutdown_rx) = mpsc::channel(1);

    // Register control handler
    let event_handler = move |control_event: ServiceControl| -> ServiceControlHandlerResult {
        match control_event {
            ServiceControl::Stop => {
                shutdown_tx.blocking_send(()).ok(); // Signal shutdown
                ServiceControlHandlerResult::NoError
            }
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    };

    let status_handle = match service_control_handler::register(SERVICE_NAME, event_handler) {
        Ok(handle) => handle,
        Err(err) => {
            error!("Failed to register service control handler: {}", err);
            return;
        }
    };

    // Report StartPending
    if let Err(err) = status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::StartPending,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::from_secs(5),
        process_id: None,
    }) {
        error!("Failed to set StartPending: {}", err);
        return;
    }

    // Run the app logic in a new runtime
    let rt = tokio::runtime::Runtime::new().unwrap();
    let app_handle = rt.spawn(app(cli, Some(shutdown_rx)));

    // Report Running after starting monitoring
    if let Err(err) = status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Running,
        controls_accepted: ServiceControlAccept::STOP,
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    }) {
        error!("Failed to set Running: {}", err);
        return;
    }

    // Wait for app to finish (on shutdown)
    rt.block_on(app_handle).ok();

    // Report Stopped
    if let Err(err) = status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Stopped,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    }) {
        error!("Failed to set Stopped: {}", err);
    }
}


async fn app(mut cli: Cli, shutdown_rx: Option<mpsc::Receiver<()>>) {
    let _guard: ClientInitGuard;
    if !cli.disable_sentry {
        _guard = sentry::init((cli.sentry_dsn.take(), sentry::ClientOptions {
            release: sentry::release_name!(),
            enable_logs: true,
            ..Default::default()
        }));
    }

    tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::try_new("info").unwrap()),
        )
        .with(tracing_subscriber::fmt::layer())
        .with(
            sentry::integrations::tracing::layer().event_filter(|md| match *md.level() {
                tracing::Level::ERROR => EventFilter::Event,
                tracing::Level::TRACE | tracing::Level::DEBUG => EventFilter::Ignore,
                _ => EventFilter::Log,
            })
        )
        .init();

    handle_installation(&cli);

    // Check if we should list monitors
    if cli.list_monitors {
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
        cli.threshold,
        monitor_indices,
    );

    if monitor.start_monitoring(cli.update_rate).await {
        info!("Started monitoring desktop visibility");

        // Unified shutdown waiting
        let ctrl_c_fut = signal::ctrl_c();
        pin!(ctrl_c_fut);

        if let Some(mut rx) = shutdown_rx {
            tokio::select! {
                _ = rx.recv() => {
                    info!("Shutdown signal received from Windows service");
                }
                res = ctrl_c_fut => {
                    match res {
                        Ok(()) => {
                            info!("Ctrl+C received");
                        }
                        Err(err) => {
                            error!("Error listening for Ctrl+C: {}", err);
                        }
                    }
                }
            }
        } else {
            // Non-service mode: only wait for Ctrl+C
            if let Err(err) = ctrl_c_fut.await {
                error!("Unable to listen for shutdown signal: {}", err);
            } else {
                info!("Ctrl+C received");
            }
        }

        if monitor.stop_monitoring().await {
            info!("Stopped monitoring desktop visibility");
        } else {
            error!("Failed to stop monitoring");
        }
    } else {
        error!("Failed to start monitoring");
    }
}