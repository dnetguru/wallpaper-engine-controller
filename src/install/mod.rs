use std::env;
use std::io::Read;
use std::{fs, thread};
use std::path::{Path, PathBuf};
use std::ffi::{OsStr, OsString};
use std::sync::mpsc;
use std::time::Duration;
use tracing::{debug, error, info, warn};

use nameof::name_of;
use clap::CommandFactory;
use windows::Win32::System::Console::{GetStdHandle, ReadConsoleW, STD_INPUT_HANDLE};
use windows_service::{
    service::{
        ServiceAccess, ServiceErrorControl, ServiceStartType, ServiceType,
    },
    service_manager::{ServiceManager, ServiceManagerAccess},
};
use windows_service::service::{Service, ServiceDependency, ServiceInfo};
use std::process::Command;

use crate::cli::Cli;

pub mod tui;

const SERVICE_NAME: &str = "WallpaperControllerService";
const SERVICE_DISPLAY_NAME: &str = "Wallpaper Controller Service";
const WALLPAPER_ENGINE_SERVICE_NAME: &str = "Wallpaper Engine Service";
const WALLPAPER_SERVICE_32_PATH: &str = "C:\\WINDOWS\\SysWOW64\\wallpaperservice32.exe";
const TASK_NAME: &str = "WallpaperControllerAtLogon";


pub fn exit_blocking(code: i32) {
    println!("Press Enter to exit...");
    let stdin_handle = unsafe { GetStdHandle(STD_INPUT_HANDLE) };
    if stdin_handle.as_ref().is_ok_and(|h| !h.is_invalid()) {
        drop(stdin_handle);
        let (tx, rx) = mpsc::channel();
        let thread = thread::spawn(move || {
            let mut read: u32 = 0;
            let mut buffer = [0u16; 1];
            let stdin_handle = unsafe { GetStdHandle(STD_INPUT_HANDLE) };
            let _ = unsafe { ReadConsoleW(stdin_handle.unwrap(), buffer.as_mut_ptr() as *mut _, buffer.len() as u32, &mut read, None) };
            tx.send(()).ok();
        });
        if rx.recv_timeout(Duration::from_secs(15)).is_err() {
            info!("Timeout waiting for user input; exiting.");
        } else {
            thread.join().ok();
        }
    }
    std::process::exit(code);
}

pub fn handle_installation(args: &Cli) {
    let mut install_path = None;
    if let Some(path_str) = &args.install_dir {
        info!("Starting installation...");
        match install_executable(path_str) {
            Ok(path) => {
                info!("Successfully installed to {}", path.display());
                install_path = Some(path);
            }
            Err(e) => {
                error!("Installation failed: {:}", e);
                exit_blocking(1);
            }
        }
    }

    if args.add_startup_service {
        let exe_path = resolve_exe_path(install_path.clone());
        let service_args = filtered_passthrough_args();

        match setup_startup_service(&exe_path, service_args) {
            Ok(svc) => {
                info!("Successfully set up the startup service.");
                if let Err(e) = svc.start::<&str>(&[]) {
                    error!("Failed to start the startup service: {}", e);
                    exit_blocking(1);
                } else {
                    info!("Service started successfully.");
                }
            },
            Err(e) => {
                error!("Failed to set up startup service: {:?}", e);
                println!("\n\n\t• Setup failed! Please try again.\n\tIf the issue persists please make sure to close all OS windows (including Task Manager, and Services) before retrying.\n");
                exit_blocking(1);
            }
        }
    }

    if args.add_startup_task {
        let exe_path = resolve_exe_path(install_path);
        let mut task_args = filtered_passthrough_args();
        // Always add `-silent` for scheduled tasks
        task_args.push(OsString::from("-silent"));

        match setup_startup_scheduled_task(&exe_path, task_args) {
            Ok(_) => {
                info!("Successfully set up the startup scheduled task.");
            },
            Err(e) => {
                error!("Failed to set up startup scheduled task: {:?}", e);
                exit_blocking(1);
            }
        }
    }

    info!("Operations completed successfully.");
    exit_blocking(0);
}

fn install_executable(target: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let current_exe = env::current_exe()?;
    let input_path = PathBuf::from(target);

    fn compute_file_hash(path: &Path) -> Result<blake3::Hash, Box<dyn std::error::Error>> {
        let mut file = fs::File::open(path)?;
        let mut hasher = blake3::Hasher::new();
        let mut buf = [0u8; 8192];
        loop {
            let read = file.read(&mut buf)?;
            if read == 0 { break; }
            hasher.update(&buf[..read]);
        }
        Ok(hasher.finalize())
    }

    // Ensure the target is a directory (existing or to be created). We do not accept file paths.
    if fs::exists(&input_path)? {
        let meta = fs::metadata(&input_path)?;
        if meta.is_file() {
            return Err(format!("Install target '{}' is a file; expected a directory", input_path.display()).into());
        }
        // It exists and is a directory
        fs::create_dir_all(&input_path)?;
    } else {
        // If the user passed a path that looks like a file (e.g., ends with .exe), reject it
        if input_path.extension().is_some_and(|ext| ext.to_string_lossy().eq_ignore_ascii_case("exe")) {
            return Err(format!("Install target '{}' appears to be a file path; please specify a directory", input_path.display()).into());
        }
        fs::create_dir_all(&input_path)?;
    }

    // Construct the final target file path using the fixed executable name
    let target_path = input_path.join("wallpaper-controller.exe");

    // If target exists, compare hashes before copying
    if fs::exists(&target_path)? {
        match (compute_file_hash(&current_exe), compute_file_hash(&target_path)) {
            (Ok(src_hash), Ok(dst_hash)) => {
                if src_hash == dst_hash {
                    info!("Same version already present at {} (hash {}).", target_path.display(), src_hash.to_hex());
                    info!("Skipping copy.");
                    return Ok(target_path);
                } else {
                    info!("Different contents detected at {}.", target_path.display());
                    info!("Updating...");
                }
            }
            (e1, e2) => {
                warn!("Failed to compute hash for comparison (src: {:?}, dst: {:?}).", e1.err(), e2.err());
                info!("Proceeding to replace file.");
            }
        }
        // Remove old file before copy
        fs::remove_file(&target_path)?;
    } else {
        info!("Installing new copy to {}", target_path.display());
    }

    fs::copy(&current_exe, &target_path)?;
    Ok(target_path)
}

fn setup_startup_service(exe_path: &Path, launch_args: Vec<OsString>) -> Result<Service, Box<dyn std::error::Error>> {
    let manager = ServiceManager::local_computer(None::<&OsStr>, ServiceManagerAccess::all())?;

    ensure_wallpaper_engine_service_present()?;

    // If switching from scheduled task to service, remove the scheduled task first
    info!("Setting up as a Windows Service.");
    if let Err(e) = remove_existing_task_if_any() {
        warn!("Failed while attempting to remove existing scheduled task '{}': {}", TASK_NAME, e);
    }

    remove_existing_service_if_any(&manager, SERVICE_NAME, Duration::from_secs(6))?;

    let mut wallpaper_service_32_args: Vec<OsString> = vec!["-p".into(), exe_path.into()];
    wallpaper_service_32_args.extend(launch_args);

    debug!("Executable: {} | Launch args: {:?}", SERVICE_NAME, wallpaper_service_32_args);

    let service_info = ServiceInfo {
        name: SERVICE_NAME.into(),
        display_name: SERVICE_DISPLAY_NAME.into(),
        service_type: ServiceType::OWN_PROCESS,
        start_type: ServiceStartType::AutoStart,
        error_control: ServiceErrorControl::Normal,
        executable_path: WALLPAPER_SERVICE_32_PATH.into(),
        launch_arguments: wallpaper_service_32_args,
        account_name: None,
        account_password: None,
        dependencies: vec![ServiceDependency::Service(WALLPAPER_ENGINE_SERVICE_NAME.into())],
    };

    // Try to create the service; if it fails (likely because deletion hasn't finalized), wait and retry once.
    match manager.create_service(&service_info, ServiceAccess::ALL_ACCESS) {
        Ok(service) => {
            info!("Service '{}' created successfully.", SERVICE_NAME);
            Ok(service)
        }
        Err(first_err) => {
            warn!("First attempt to create service '{}' failed: {}", SERVICE_NAME, first_err);
            info!("Waiting several more seconds before retrying service creation...");
            thread::sleep(Duration::from_secs(6));
            match manager.create_service(&service_info, ServiceAccess::ALL_ACCESS) {
                Ok(service) => {
                    info!("Service '{}' created successfully on retry.", SERVICE_NAME);
                    Ok(service)
                }
                Err(e) => {
                    error!("Second attempt to create service '{}' failed: {}", SERVICE_NAME, e);
                    Err(Box::new(e))
                }
            }
        }
    }
}

fn quote_arg<S: AsRef<OsStr>>(s: S) -> OsString {
    let s_ref = s.as_ref();
    let s_str = s_ref.to_string_lossy();
    if s_str.chars().any(|c| c.is_whitespace()) || s_str.contains(['"', '^', '&', '|', '>', '<']) {
        let mut q = OsString::from("\"");
        q.push(&*s_str.replace('"', "\\\""));
        q.push("\"");
        q
    } else {
        s_ref.to_owned()
    }
}

fn setup_startup_scheduled_task(exe_path: &Path, launch_args: Vec<OsString>) -> Result<(), Box<dyn std::error::Error>> {
    let username = std::env::var("USERNAME").unwrap_or_else(|_| String::from("%USERNAME%"));

    // If switching from service to scheduled task, remove the service first
    info!("Setting up as a Scheduled Task.");
    info!("If a startup service installation exists, it will be removed.");
    let manager = ServiceManager::local_computer(None::<&OsStr>, ServiceManagerAccess::all())?;
    if let Err(e) = remove_existing_service_if_any(&manager, SERVICE_NAME, Duration::from_secs(6)) {
        warn!("Failed while attempting to remove existing service '{}': {}", SERVICE_NAME, e);
    }

    fn build_command_line(exe_path: &Path, args: &[OsString]) -> OsString {
        let mut full_cmd = OsString::new();
        full_cmd.push(quote_arg(exe_path.as_os_str()));
        for a in args {
            full_cmd.push(" ");
            full_cmd.push(quote_arg(a));
        }
        full_cmd
    }

    // Create or update the task
    let output = Command::new("schtasks")
        .args([
            "/Create", "/TN", TASK_NAME,
            "/TR",
        ])
        .arg(build_command_line(exe_path, &launch_args))
        .args([
            "/SC", "ONLOGON",
            "/RL", "HIGHEST",
            "/RU", &username,
            "/DELAY", "0001:15",
            "/F",
        ])
        .output()?;

    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        warn!("schtasks output: {}", stdout);
        error!("schtasks error: {}", stderr);
        return Err(format!("schtasks /Create failed with code {:?}", output.status.code()).into());
    }

    Ok(())
}

fn resolve_exe_path(install_path: Option<PathBuf>) -> PathBuf {
    install_path.unwrap_or_else(|| env::current_exe().expect("Failed to get current exe path"))
}

fn filtered_passthrough_args() -> Vec<OsString> {
    // List of parameters to skip when passing through to the service/task (second arg is whether it takes a value)
    let skip = [
        (name_of!(install_tui in Cli), false),
        (name_of!(install_dir in Cli), true),
        (name_of!(add_startup_service in Cli), false),
        (name_of!(add_startup_task in Cli), false),
    ];

    let cmd = Cli::command();
    let mut skip_flags: Vec<(String, bool)> = Vec::with_capacity(skip.len());
    for (field_name, takes_value) in skip {
        if let Some(arg) = cmd.get_arguments().find(|a| a.get_id() == field_name) {
            if let Some(long) = arg.get_long() {
                skip_flags.push((format!("--{}", long), takes_value));
            }
        }
    }

    let mut out: Vec<OsString> = vec![];
    let mut args_iter = std::env::args_os().skip(1);
    'outer: while let Some(arg) = args_iter.next() {
        let arg_str = arg.to_string_lossy();
        for (flag, takes_value) in &skip_flags {
            let eq_prefix = format!("{}=", flag);
            if &arg_str == flag {
                if *takes_value {
                    let _ = args_iter.next(); // skip value
                }
                continue 'outer;
            } else if arg_str.starts_with(&eq_prefix) {
                continue 'outer;
            }
        }
        out.push(arg);
    }
    out
}

fn ensure_wallpaper_engine_service_present() -> Result<(), Box<dyn std::error::Error>> {
    if !fs::exists(&WALLPAPER_SERVICE_32_PATH)? {
        error!("Running this application as a service requires `wallpaperservice32.exe` to have been installed as part of Wallpaper Engine.");
        info!("You can try setting Wallpaper Engine to run as a service OR use a scheduled task to run this application on startup.");
        return Err("wallpaperservice32.exe not found".into());
    }
    Ok(())
}

fn remove_existing_service_if_any(manager: &ServiceManager, name: &str, wait_after_delete: Duration) -> Result<(), Box<dyn std::error::Error>> {
    if let Ok(service) = manager.open_service(name, ServiceAccess::all()) {
        info!("Service '{}' already exists. Trying to delete it.", name);
        let _ = service.stop();
        if let Err(e) = service.delete() {
            error!("Failed to delete service '{}'.", name);
            error!("You might need to close Services and Task Manager windows and/or log out from or restart your computer to proceed");
            error!("Error: {}", e);
            exit_blocking(2);
        } else {
            info!("Service '{}' was marked for deletion successfully.", name);
            info!("Waiting several seconds before continuing...");
            thread::sleep(wait_after_delete);
        }
    }
    Ok(())
}

fn remove_existing_task_if_any() -> Result<(), Box<dyn std::error::Error>> {
    // Check if the scheduled task exists and delete it if it does.
    info!("Checking for existing scheduled task '{}'...", TASK_NAME);
    let query = Command::new("schtasks")
        .args(["/Query", "/TN", TASK_NAME])
        .output()?;

    if query.status.success() {
        info!("Scheduled task '{}' found. Attempting to delete it...", TASK_NAME);
        let delete_out = Command::new("schtasks")
            .args(["/Delete", "/TN", TASK_NAME, "/F"]).output()?;
        if delete_out.status.success() {
            info!("Scheduled task '{}' deleted successfully.", TASK_NAME);
        } else {
            let stdout = String::from_utf8_lossy(&delete_out.stdout);
            let stderr = String::from_utf8_lossy(&delete_out.stderr);
            warn!("Failed to delete scheduled task '{}'. stdout: {}", TASK_NAME, stdout);
            error!("stderr: {}", stderr);
            return Err(format!("Failed to delete scheduled task '{}' with code {:?}", TASK_NAME, delete_out.status.code()).into());
        }
    } else {
        debug!("Scheduled task '{}' not found; nothing to remove.", TASK_NAME);
    }

    Ok(())
}
