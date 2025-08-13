use libvisdesk::{LibVisInstance, MonitorVisibleInfo};
use std::collections::HashMap;
use std::sync::Arc;
use log::{info, debug, error, warn};
use tokio::sync::{mpsc, Mutex};
use crate::wallpaper::WallpaperController;
use crate::cli::Mode;

// Define our own message type for the monitor channel
enum MonitorMessage {
    VisibilityUpdate(Vec<MonitorVisibleInfo>, i64, i64),
    Shutdown,
}

pub struct VisibilityMonitor {
    instance: LibVisInstance,
    controller: Arc<Mutex<WallpaperController>>,
    mode: Mode,
    global_threshold: u8,
    monitor_thresholds: HashMap<i64, u8>,
    monitor_ids: Option<Vec<i64>>,
    tx: Option<mpsc::Sender<MonitorMessage>>,
    running: bool,
}

impl VisibilityMonitor {
    pub fn new(
        controller: WallpaperController,
        mode: Mode,
        global_threshold: u8,
        monitor_thresholds: HashMap<i64, u8>,
        monitor_ids: Option<Vec<i64>>,
    ) -> Self {
        Self {
            instance: LibVisInstance::new(),
            controller: Arc::new(Mutex::new(controller)),
            mode,
            global_threshold,
            monitor_thresholds,
            monitor_ids,
            tx: None,
            running: false,
        }
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
        let mode = self.mode;
        let global_threshold = self.global_threshold;
        let monitor_thresholds = self.monitor_thresholds.clone();
        let monitor_ids = self.monitor_ids.clone();

        tokio::spawn(async move {
            Self::process_visibility_updates(
                rx, 
                controller, 
                mode, 
                global_threshold, 
                monitor_thresholds
            ).await;
        });

        // Set up the callback to forward messages to our channel
        let tx_clone = self.tx.clone().unwrap();
        let callback = move |monitors: &[MonitorVisibleInfo], total_visible: i64, total_area: i64, _: *mut std::ffi::c_void| {
            // Filter monitors if specific IDs were provided
            let filtered_monitors = if let Some(ids) = &monitor_ids {
                monitors.iter()
                    .filter(|m| ids.contains(&m.monitor_id))
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
                total_visible,
                total_area
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
        mode: Mode,
        global_threshold: u8,
        monitor_thresholds: HashMap<i64, u8>,
    ) {
        while let Some(message) = rx.recv().await {
            match message {
                MonitorMessage::VisibilityUpdate(monitors, _total_visible, _total_area) => {
                    match mode {
                        Mode::Global => {
                            // Calculate total visibility percentage across all monitored displays
                            let mut monitored_visible = 0;
                            let mut monitored_total = 0;
                            
                            for monitor in &monitors {
                                monitored_visible += monitor.current_visible;
                                monitored_total += monitor.total_area;
                            }
                            
                            let visibility_percent = if monitored_total > 0 {
                                (monitored_visible as f64 / monitored_total as f64 * 100.0) as u8
                            } else {
                                0
                            };
                            
                            debug!("Global visibility: {}%", visibility_percent);
                            
                            let mut controller_lock = controller.lock().await;
                            if visibility_percent < global_threshold && controller_lock.is_playing(None) {
                                info!("Global visibility below threshold ({}%), pausing Wallpaper Engine", global_threshold);
                                controller_lock.pause(None).await;
                            } else if visibility_percent >= global_threshold && !controller_lock.is_playing(None) {
                                info!("Global visibility above threshold ({}%), resuming Wallpaper Engine", global_threshold);
                                controller_lock.play(None).await;
                            }
                        },
                        Mode::PerMonitor => {
                            let mut controller_lock = controller.lock().await;
                            
                            for monitor in &monitors {
                                let threshold = monitor_thresholds.get(&monitor.monitor_id)
                                    .unwrap_or(&global_threshold);
                                    
                                let visibility_percent = if monitor.total_area > 0 {
                                    (monitor.current_visible as f64 / monitor.total_area as f64 * 100.0) as u8
                                } else {
                                    0
                                };
                                
                                debug!("Monitor {} visibility: {}%", monitor.monitor_id, visibility_percent);
                                
                                if visibility_percent < *threshold && controller_lock.is_playing(Some(monitor.monitor_id)) {
                                    info!("Monitor {} visibility below threshold ({}%), pausing", 
                                          monitor.monitor_id, threshold);
                                    controller_lock.pause(Some(monitor.monitor_id)).await;
                                } else if visibility_percent >= *threshold && !controller_lock.is_playing(Some(monitor.monitor_id)) {
                                    info!("Monitor {} visibility above threshold ({}%), resuming", 
                                          monitor.monitor_id, threshold);
                                    controller_lock.play(Some(monitor.monitor_id)).await;
                                }
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

        // Send shutdown message
        if let Some(tx) = &self.tx {
            let _ = tx.send(MonitorMessage::Shutdown).await;
        }

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