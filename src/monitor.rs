use std::sync::Arc;
use std::collections::HashMap;
use tracing::{info, debug, error, warn};
use tokio::sync::{mpsc, Mutex};
use libvisdesk::{LibVisInstance, MonitorVisibleInfo};

use crate::wallpaper::WallpaperController;

// Define our own message type for the monitor channel
enum MonitorMessage {
    VisibilityUpdate(Vec<MonitorVisibleInfo>),
    Shutdown,
}

pub struct VisibilityMonitor {
    instance: LibVisInstance,
    controller: Arc<Mutex<WallpaperController>>,
    per_monitor: bool,
    threshold: u8,
    monitor_indices: Option<Vec<i64>>,
    tx: Option<mpsc::Sender<MonitorMessage>>,
    running: bool,
}

impl VisibilityMonitor {
    pub fn new(
        controller: WallpaperController,
        per_monitor: bool,
        threshold: u8,
        monitor_indices: Option<Vec<i64>>,
    ) -> Self {
        Self {
            instance: LibVisInstance::new(),
            controller: Arc::new(Mutex::new(controller)),
            per_monitor,
            threshold,
            monitor_indices,
            tx: None,
            running: false,
        }
    }
    
    pub async fn get_controller(&'_ self) -> tokio::sync::MutexGuard<'_, WallpaperController> {
        // Return a reference to the existing controller
        self.controller.lock().await
    }

    pub async fn start_monitoring(&mut self, throttle_ms: u64) -> bool {
        if self.running {
            warn!("Already monitoring");
            return false;
        }

        // Create a channel for communication
        let (tx, rx) = mpsc::channel::<MonitorMessage>(100);
        self.tx = Some(tx);

        // Start the processor task
        let controller = Arc::clone(&self.controller);
        let per_monitor = self.per_monitor;
        let threshold = self.threshold;
        let monitor_indices = self.monitor_indices.clone();

        tokio::spawn(async move {
            Self::process_visibility_updates(
                rx, 
                controller, 
                per_monitor, 
                threshold
            ).await;
        });

        // Set up the callback to forward messages to our channel
        let tx_clone = self.tx.clone().unwrap();
        let callback = move |monitors: &[MonitorVisibleInfo], _total_visible: i64, _total_area: i64, _: *mut std::ffi::c_void| {
            // Filter monitors if specific indices were provided
            let filtered_monitors = if let Some(indices) = &monitor_indices {
                monitors.iter()
                    .filter(|m| indices.contains(&m.monitor_index))
                    .cloned()
                    .collect::<Vec<_>>()
            } else {
                monitors.to_vec()
            };
            
            if filtered_monitors.is_empty() {
                return;
            }
            
            // Clone the data and send it through the channel
            let message = MonitorMessage::VisibilityUpdate(
                filtered_monitors,
            );
            
            // Use try_send to avoid blocking in the callback
            if let Err(e) = tx_clone.try_send(message) {
                error!("Failed to send visibility update: {}", e);
            }
        };

        // Start watching with libvisdesk
        if self.instance.watch_visible_area(callback, throttle_ms, std::ptr::null_mut()) {
            self.running = true;
            info!("Started monitoring desktop visibility");
            true
        } else {
            error!("Failed to start monitoring");
            false
        }
    }

    async fn process_visibility_updates(
        mut rx: mpsc::Receiver<MonitorMessage>,
        controller: Arc<Mutex<WallpaperController>>,
        per_monitor: bool,
        threshold: u8,
    ) {
        // Create local tracking variables for this function instance
        let mut previous_global_visibility: Option<u8> = None;
        let mut previous_monitor_visibilities: HashMap<i64, u8> = HashMap::new();
        
        while let Some(message) = rx.recv().await {
            match message {
                MonitorMessage::VisibilityUpdate(monitors) => {
                    if !per_monitor {
                        // Global mode - Calculate total visibility percentage across all monitored displays
                        let mut monitored_visible = 0;
                        let mut monitored_total = 0;
                        
                        for monitor in &monitors {
                            monitored_visible += monitor.current_visible;
                            monitored_total += monitor.max_visible;
                        }
                        
                        let visibility_percent = if monitored_total > 0 {
                            (monitored_visible as f64 / monitored_total as f64 * 100.0) as u8
                        } else {
                            0
                        };
                        
                        debug!("Global visibility: {}%", visibility_percent);
                        
                        let mut controller_lock = controller.lock().await;
                        
                        // Check if we crossed the threshold in either direction
                        let crossed_threshold_down = visibility_percent < threshold && 
                            (previous_global_visibility.is_none() || previous_global_visibility.unwrap() >= threshold);
                        let crossed_threshold_up = visibility_percent >= threshold && 
                            (previous_global_visibility.is_none() || previous_global_visibility.unwrap() < threshold);
                        
                        // Update previous visibility
                        previous_global_visibility = Some(visibility_percent);
                        
                        if crossed_threshold_down && controller_lock.is_playing(None) {
                            info!("Global visibility {visibility_percent} is below threshold ({threshold}%), pausing Wallpaper Engine");
                            controller_lock.pause(None).await;
                        } else if crossed_threshold_up && !controller_lock.is_playing(None) {
                            info!("Global visibility {visibility_percent} is above threshold ({threshold}%), resuming Wallpaper Engine");
                            controller_lock.play(None).await;
                        }
                    } else {
                        // Per-monitor mode - Apply the same threshold to each monitor
                        let mut controller_lock = controller.lock().await;
                        
                        for monitor in &monitors {
                            let visibility_percent = if monitor.max_visible > 0 {
                                (monitor.current_visible as f64 / monitor.max_visible as f64 * 100.0) as u8
                            } else {
                                0
                            };
                            
                            debug!("Monitor number {} visibility: {}%", monitor.monitor_index, visibility_percent);
                            
                            // Get previous visibility for this monitor
                            let previous_visibility = previous_monitor_visibilities.get(&monitor.monitor_index).cloned();
                            
                            // Check if we crossed the threshold in either direction
                            let crossed_threshold_down = visibility_percent < threshold && 
                                (previous_visibility.is_none() || previous_visibility.unwrap() >= threshold);
                            let crossed_threshold_up = visibility_percent >= threshold && 
                                (previous_visibility.is_none() || previous_visibility.unwrap() < threshold);
                            
                            // Update previous visibility for this monitor
                            previous_monitor_visibilities.insert(monitor.monitor_index, visibility_percent);
                            
                            if crossed_threshold_down && controller_lock.is_playing(Some(monitor.monitor_index)) {
                                info!("Monitor number {} visibility below threshold ({}%), pausing",
                                      monitor.monitor_index, threshold);
                                controller_lock.pause(Some(monitor.monitor_index)).await;
                            } else if crossed_threshold_up && !controller_lock.is_playing(Some(monitor.monitor_index)) {
                                info!("Monitor number {} visibility above threshold ({}%), resuming",
                                      monitor.monitor_index, threshold);
                                controller_lock.play(Some(monitor.monitor_index)).await;
                            }
                        }
                    }
                },
                MonitorMessage::Shutdown => {
                    info!("Received shutdown message");
                    break;
                }
            }
        }
        
        info!("Visibility update processor stopped");
    }

    pub async fn stop_monitoring(&mut self) -> bool {
        if !self.running {
            warn!("Not monitoring");
            return false;
        }

        // Send a shutdown message
        if let Some(tx) = &self.tx {
            let _ = tx.send(MonitorMessage::Shutdown).await;
        }

        // Resume wallpapers before stopping the watcher
        {
            let mut controller = self.get_controller().await;
            if let Some(ref indices) = self.monitor_indices {
                for &i in indices.iter() { controller.play(Some(i)).await; }
            } else {
                controller.play(None).await;
            }
        } // Release the lock on the controller here

        info!("Resumed all wallpapers...");

        // Stop the libvisdesk watcher
        if self.instance.stop_watch_visible_area() {
            self.running = false;
            info!("Stopped monitoring desktop visibility");
            true
        } else {
            error!("Failed to stop monitoring");
            false
        }
    }
}