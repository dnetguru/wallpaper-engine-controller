use clap::Parser;
use tracing::warn;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// Monitors to watch (comma-separated IDs, or "all" for all monitors)
    #[arg(short, long, default_value = "all")]
    pub monitors: String,

    /// Minimum visibility threshold percentage (0-100) to pause the wallpaper engine
    #[arg(short, long, default_value_t = 20, value_parser = clap::value_parser!(u8).range(0..=100))]
    pub threshold: u8,

    /// Per-monitor mode - track visibility for each monitor separately
    #[arg(short='p', long="per-monitor")]
    pub per_monitor: bool,

    /// Maximum update frequency in milliseconds
    #[arg(short, long, default_value_t = 1000)]
    pub update_rate: u64,

    /// Path to Wallpaper Engine executable
    #[arg(long, default_value = "C:\\Program Files (x86)\\Steam\\steamapps\\common\\wallpaper_engine")]
    pub wallpaper_engine_path: String,
    
    /// Use the 64-bit version of Wallpaper Engine (wallpaper64.exe), otherwise use 32-bit (wallpaper32.exe)
    #[arg(long="64bit")]
    pub bit64: bool,
    
    /// List all available monitors and their IDs, then exit
    #[arg(short='L', long="list-monitors")]
    pub list_monitors: bool,

    /// Disable Sentry error reporting
    #[arg(long, default_value = "false")]
    pub disable_sentry: bool,

    /// Override the default Sentry error reporting DSN (for debugging purposes)
    #[arg(long, default_value = "https://c6caa06487e9769daccfbedcd8de6324@o504783.ingest.us.sentry.io/4509839881076736")]
    pub sentry_dsn: String,
}

pub fn parse_monitor_ids(input: &str) -> Option<Vec<i64>> {
    if input.to_lowercase() == "all" {
        return None; // None represents all monitors
    }
    
    // Get the list of all monitors to map indices to actual IDs
    let instance = libvisdesk::LibVisInstance::new();
    let (monitors, _, _) = instance.get_visible_area();
    
    let mut ids = Vec::new();
    for id_str in input.split(',') {
        if let Ok(index) = id_str.trim().parse::<usize>() {
            // Convert index to actual monitor ID
            if index < monitors.len() {
                ids.push(monitors[index].monitor_id);
            } else {
                // Log a warning if index is out of bounds
                warn!("Monitor index {} is out of bounds, ignoring", index);
            }
        }
    }
    
    Some(ids)
}