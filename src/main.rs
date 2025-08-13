mod cli;
mod monitor;
mod wallpaper;

use clap::Parser;
use env_logger::Env;
use log::{info, error};
use tokio::signal;
use cli::{Cli, parse_monitor_thresholds, parse_monitor_ids};
use monitor::VisibilityMonitor;
use wallpaper::WallpaperController;

#[tokio::main]
async fn main() {
    let env = Env::default()
        .filter_or("WC_LOG_LEVEL", "debug");
    env_logger::init_from_env(env);

    let cli = Cli::parse();
    
    // Validate threshold
    if cli.threshold > 100 {
        error!("Threshold must be between 0 and 100");
        return;
    }
    
    // Parse monitor IDs
    let monitor_ids = parse_monitor_ids(&cli.monitors);
    
    // Parse per-monitor thresholds if provided
    let monitor_thresholds = if let Some(thresholds_str) = &cli.monitor_thresholds {
        parse_monitor_thresholds(thresholds_str)
    } else {
        std::collections::HashMap::new()
    };
    
    // Create wallpaper controller with the 64-bit flag
    let controller = WallpaperController::new(cli.wallpaper_path, cli.bit64);
    
    // Create and start visibility monitor
    let mut monitor = VisibilityMonitor::new(
        controller,
        cli.mode,
        cli.threshold,
        monitor_thresholds,
        monitor_ids,
    );
    
    if monitor.start_monitoring(cli.update_rate).await {
        info!("Started monitoring desktop visibility");
        info!("Press Ctrl+C to stop...");
        
        // Wait for Ctrl+C
        match signal::ctrl_c().await {
            Ok(()) => {
                info!("Ctrl+C received, shutting down...");
            }
            Err(err) => {
                error!("Unable to listen for shutdown signal: {}", err);
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