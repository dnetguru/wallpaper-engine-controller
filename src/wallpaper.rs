use std::path::Path;
use std::collections::HashMap;
use std::time::Duration;
use tracing::{info, error, debug};
use tokio::process::Command as TokioCommand;
use tokio::time::timeout;

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

        // Use tokio::process for async execution with timeout
        let mut child = match TokioCommand::new(&full_path)
            .args(&args)
            .spawn() {
            Ok(child) => child,
            Err(e) => {
                error!("Failed to spawn command: {}", e);
                return false;
            }
        };

        let wait_timeout = Duration::from_secs(5);
        let wait_result = timeout(wait_timeout, child.wait()).await;

        let success = match wait_result {
            Ok(Ok(status)) => status.success(),
            Ok(Err(e)) => {
                error!("Failed to wait for child process: {}", e);
                false
            }
            Err(_) => {  // Timeout occurred
                error!("Child process timed out after {:?}; attempting to kill", wait_timeout);
                if let Err(kill_err) = child.kill().await {
                    error!("Failed to kill timed-out child process: {}", kill_err);
                }
                false
            }
        };

        // Update state tracking
        match monitor_index {
            Some(index) => {
                self.monitor_states.insert(index, command == "play");
            },
            None => {
                self.global_state = command == "play";
            }
        }

        success
    }

    pub fn is_playing(&self, monitor_index: Option<i64>) -> bool {
        match monitor_index {
            Some(index) => *self.monitor_states.get(&index).unwrap_or(&true),
            None => self.global_state,
        }
    }
}