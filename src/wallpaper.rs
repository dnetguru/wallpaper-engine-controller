use std::path::Path;
use std::collections::HashMap;
use tracing::{info, error, debug};
use libvisdesk::LibVisInstance;

#[derive(Clone)]
pub struct WallpaperController {
    executable_path: String,
    use_64bit: bool,
    global_state: bool, // true = playing, false = paused
    monitor_states: HashMap<i64, bool>,
    monitor_id_to_index: HashMap<i64, usize>,
}

impl WallpaperController {
    pub fn new(base_path: String, use_64bit: bool) -> Self {
        let instance = LibVisInstance::new();
        let (monitors, _, _) = instance.get_visible_area();
        
        let mut monitor_id_to_index = HashMap::new();
        
        for (index, monitor) in monitors.iter().enumerate() {
            monitor_id_to_index.insert(monitor.monitor_id, index);
        }
        
        Self {
            executable_path: base_path,
            use_64bit,
            global_state: true, // Assume wallpaper is playing initially
            monitor_states: HashMap::new(),
            monitor_id_to_index,
        }
    }

    pub async fn pause(&mut self, monitor_id: Option<i64>) -> bool {
        self.execute_command("pause", monitor_id).await
    }

    pub async fn play(&mut self, monitor_id: Option<i64>) -> bool {
        self.execute_command("play", monitor_id).await
    }

    async fn execute_command(&mut self, command: &str, monitor_id: Option<i64>) -> bool {
        let mut args = vec![String::from("-control"), String::from(command)];
        
        // Add monitor ID if specified
        if let Some(id) = monitor_id {
            // Convert system ID to user-friendly index for wallpaper engine
            let monitor_index = self.monitor_id_to_index.get(&id).cloned().unwrap_or(0);
            
            args.push(String::from("-monitor"));
            args.push(monitor_index.to_string());
            
            debug!("Using monitor index {} (system ID: {}) for command", monitor_index, id);
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
        match monitor_id {
            Some(id) => {
                self.monitor_states.insert(id, command == "play");
            },
            None => {
                self.global_state = command == "play";
            }
        }
        
        status
    }
    
    pub fn is_playing(&self, monitor_id: Option<i64>) -> bool {
        match monitor_id {
            Some(id) => *self.monitor_states.get(&id).unwrap_or(&true),
            None => self.global_state,
        }
    }
}