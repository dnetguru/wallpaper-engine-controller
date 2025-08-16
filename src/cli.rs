use clap::Parser;
use tracing::warn;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// Monitors to watch, use numbers shown in Display Settings or use -L to list monitors (comma-separated, or "all" for all monitors)
    #[arg(short, long, default_value = "all")]
    pub monitors: String,

    /// Minimum visibility threshold percentage (0-100) to pause the wallpaper engine
    #[arg(short, long, default_value_t = 20, value_parser = clap::value_parser!(u8).range(0..=100))]
    pub threshold: u8,

    /// Per-monitor mode - track visibility for each monitor separately (THIS IS NOT SUPPORTED BY WALLPAPER ENGINE, YET)
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
    pub sentry_dsn: Option<String>,

    /// Install the assembly to the specified path and exit
    #[arg(long)]
    pub install: Option<String>,

    /// Add a Windows service to run this program with the specified flags and exit
    #[arg(long)]
    pub add_startup_service: bool,
}

pub fn parse_monitor_indices(input: &str) -> Option<Vec<i64>> {
    if input.to_lowercase() == "all" {
        return None; // None represents all monitors
    }
    
    let mut indices = Vec::new();
    for id_str in input.split(',') {
        if let Ok(index) = id_str.trim().parse::<i64>() {
            indices.push(index);
        } else {
            warn!("Invalid monitor index '{}', ignoring", id_str);
        }
    }
    
    Some(indices)
}