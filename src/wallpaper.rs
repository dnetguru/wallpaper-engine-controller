use std::path::Path;
use std::collections::HashMap;
use tracing::{info, error, debug};

#[derive(Clone)]
pub struct WallpaperController {
    executable_path: String,
    use_64bit: bool,
    global_state: bool, // true = playing, false = paused
    monitor_states: HashMap<i64, bool>,
}

impl WallpaperController {
    pub fn new(base_path: String, use_64bit: bool) -> Self {
        Self {
            executable_path: base_path,
            use_64bit,
            global_state: true, // Assume wallpaper is playing initially
            monitor_states: HashMap::new(),
        }
    }

    pub async fn pause(&mut self, monitor_index: Option<i64>) -> bool {
        self.execute_command("pause", monitor_index).await
    }

    pub async fn play(&mut self, monitor_index: Option<i64>) -> bool {
        self.execute_command("play", monitor_index).await
    }

    async fn execute_command(&mut self, command: &str, monitor_index: Option<i64>) -> bool {
        let mut args = vec![String::from("-control"), String::from(command)];
        
        // Add monitor index if specified
        if let Some(index) = monitor_index {
            args.push(String::from("-monitor"));
            args.push(index.to_string());
            debug!("Using monitor index {} for command", index);
        }
        
        // Determine which executable to use based on the 64-bit flag
        let executable_name = if self.use_64bit {
            "wallpaper64.exe"
        } else {
            "wallpaper32.exe"
        };
        
        let full_path = Path::new(&self.executable_path).join(executable_name);
        let full_path_str = full_path.to_string_lossy().to_string();
        
        info!("Executing: {} {}", full_path_str, args.join(" "));
        
        // Use tokio::process for async execution
        let status = tokio::task::spawn_blocking(move || {
            match std::process::Command::new(full_path)
                .args(&args)
                .spawn() {
                    Ok(mut child) => {
                        match child.wait() {
                            Ok(status) => status.success(),
                            Err(e) => {
                                error!("Failed to wait for child process: {}", e);
                                false
                            }
                        }
                    },
                    Err(e) => {
                        error!("Failed to execute command: {}", e);
                        false
                    }
                }
        }).await.unwrap_or(false);
        
        // Update state tracking
        match monitor_index {
            Some(index) => {
                self.monitor_states.insert(index, command == "play");
            },
            None => {
                self.global_state = command == "play";
            }
        }
        
        status
    }
    
    pub fn is_playing(&self, monitor_index: Option<i64>) -> bool {
        match monitor_index {
            Some(index) => *self.monitor_states.get(&index).unwrap_or(&true),
            None => self.global_state,
        }
    }
}