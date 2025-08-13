use clap::{Parser, ValueEnum};
use std::collections::HashMap;

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Debug)]
pub enum Mode {
    /// Global mode - track total visibility across all monitored displays
    Global,
    /// Per-monitor mode - track visibility for each monitor separately
    PerMonitor,
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// Monitors to watch (comma-separated IDs, or "all" for all monitors)
    #[arg(short, long, default_value = "all")]
    pub monitors: String,

    /// Visibility threshold percentage (0-100) to trigger pause/resume
    #[arg(short, long, default_value_t = 50)]
    pub threshold: u8,

    /// Operation mode (global or per-monitor)
    #[arg(short='x', long, value_enum, default_value_t = Mode::Global)]
    pub mode: Mode,

    /// Per-monitor thresholds (format: "monitor_id:threshold,...")
    #[arg(long="thresholds")]
    pub monitor_thresholds: Option<String>,

    /// Update frequency in milliseconds
    #[arg(short, long, default_value_t = 500)]
    pub update_rate: u64,

    /// Path to Wallpaper Engine executable
    #[arg(long, default_value = "C:\\Program Files (x86)\\Steam\\steamapps\\common\\wallpaper_engine")]
    pub wallpaper_path: String,
    
    /// Use the 64-bit version of Wallpaper Engine (wallpaper64.exe), otherwise use 32-bit (wallpaper32.exe)
    #[arg(long)]
    pub bit64: bool,
}

pub fn parse_monitor_thresholds(input: &str) -> HashMap<i64, u8> {
    let mut result = HashMap::new();
    
    for pair in input.split(',') {
        if let Some((id_str, threshold_str)) = pair.split_once(':') {
            if let (Ok(id), Ok(threshold)) = (id_str.trim().parse::<i64>(), threshold_str.trim().parse::<u8>()) {
                if threshold <= 100 {
                    result.insert(id, threshold);
                }
            }
        }
    }
    
    result
}

pub fn parse_monitor_ids(input: &str) -> Option<Vec<i64>> {
    if input.to_lowercase() == "all" {
        return None; // None represents all monitors
    }
    
    let mut ids = Vec::new();
    for id_str in input.split(',') {
        if let Ok(id) = id_str.trim().parse::<i64>() {
            ids.push(id);
        }
    }
    
    Some(ids)
}