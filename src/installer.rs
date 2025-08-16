use std::{fs, thread};
use std::env;
use std::path::{Path, PathBuf};
use std::ffi::{OsStr, OsString};
use std::sync::mpsc;
use std::time::Duration;
use tracing::{debug, error, info};

use nameof::name_of;
use clap::CommandFactory;
use windows::Win32::System::Console::{FreeConsole, GetStdHandle, ReadConsoleW, STD_INPUT_HANDLE};
use windows_elevate::{check_elevated, elevate};
use windows_service::{
    service::{
        ServiceAccess, ServiceErrorControl, ServiceStartType, ServiceType,
    },
    service_manager::{ServiceManager, ServiceManagerAccess},
};
use windows_service::service::{Service, ServiceDependency, ServiceInfo};

use crate::cli::Cli;

const SERVICE_NAME: &str = "WallpaperControllerService";
const SERVICE_DISPLAY_NAME: &str = "Wallpaper Controller Service";
const WALLPAPER_ENGINE_SERVICE_NAME: &str = "Wallpaper Engine Service";
const WALLPAPER_SERVICE_32_PATH: &str = "C:\\WINDOWS\\SysWOW64\\wallpaperservice32.exe";


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
        if rx.recv_timeout(Duration::from_secs(10)).is_err() {
            info!("Timeout waiting for user input; exiting.");
        } else {
            thread.join().ok();
        }
    }
    if !check_elevated().unwrap_or(false) { unsafe { FreeConsole() }.ok(); }
    std::process::exit(code);
}

pub fn handle_installation(args: &Cli) {
    if !check_elevated().unwrap_or(false) {
        info!("Requesting administrator privileges...");
        info!("Process will continue in a new window");

        if let Err(e) = elevate() {
            error!("Failed to elevate process: {:?}", e);
        }

        std::process::exit(0); // Exit the non-elevated process
    }

    let mut install_path = None;
    if let Some(path_str) = &args.install {
        info!("Starting installation...");
        match install_executable(path_str) {
            Ok(path) => {
                info!("Successfully installed to {}", path.display());
                install_path = Some(path);
            }
            Err(e) => {
                error!("Installation failed (does the file already exist and a process running?): {:}", e);
                exit_blocking(1);
            }
        }
    }

    if args.add_startup_service {
        let mut service_args: Vec<OsString> = vec![];
        let exe_path = install_path.unwrap_or_else(|| env::current_exe().expect("Failed to get current exe path"));

        {
            let cmd = Cli::command();
            let install_arg = cmd
                .get_arguments()
                .find(|a| a.get_id() == name_of!(install in Cli))
                .unwrap();
            let add_service_arg = cmd
                .get_arguments()
                .find(|a| a.get_id() == name_of!(add_startup_service in Cli))
                .unwrap();

            let install_flag = format!("--{}", install_arg.get_long().unwrap());
            let install_flag_eq = format!("--{}=", install_arg.get_long().unwrap());
            let add_service_flag = format!("--{}", add_service_arg.get_long().unwrap());

            let mut args_iter = std::env::args_os().skip(1);
            while let Some(arg) = args_iter.next() {
                if arg == install_flag.as_str() {
                    // This option takes a value, so we skip the next argument as well.
                    // This assumes the value is passed as a separate argument.
                    args_iter.next();
                } else if arg == add_service_flag.as_str() || arg.to_string_lossy().starts_with(install_flag_eq.as_str()) {
                    // This handles `--startup-service` and `--install=value` form so we just skip this argument.
                    continue;
                } else {
                    service_args.push(arg);
                }
            }
        }

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
                exit_blocking(1);
            }
        }
    }

    info!("Operations completed successfully.");
    exit_blocking(0);
}

fn install_executable(target: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let current_exe = env::current_exe()?;
    let target_path = PathBuf::from(target);

    // TODO: Check if the path is a directory, if so append the assembly name

    if fs::exists(&target_path)? {
        fs::remove_file(&target_path)?;
    }

    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::copy(&current_exe, &target_path)?;
    Ok(target_path)
}

fn setup_startup_service(exe_path: &Path, launch_args: Vec<OsString>) -> Result<Service, Box<dyn std::error::Error>> {
    let manager = ServiceManager::local_computer(None::<&OsStr>, ServiceManagerAccess::all())?;

    if !fs::exists(&WALLPAPER_SERVICE_32_PATH)? {
        error!("Running this application as a service requires `wallpaperservice32.exe` to have been installed as part of Wallpaper Engine.");
        info!("You can try setting Wallpaper Engine to run as a service OR use a scheduled task to run this application on startup.");
        return Err("wallpaperservice32.exe not found".into());
    }

    if let Ok(service) = manager.open_service(SERVICE_NAME, ServiceAccess::all()) {
        info!("Service '{}' already exists. Trying to delete it.", SERVICE_NAME);
        let _ = service.stop(); // Try and stop first, in case it's running
        if let Err(e) = service.delete() {
            error!("Failed to delete service '{}'. You might need to close Services and Task Manager windows and/or log out from or restart your computer to proceed", SERVICE_NAME);
            error!("Error: {}", e);
            exit_blocking(2);
        } else {
            info!("Service '{}' was marked for deletion successfully.", SERVICE_NAME);
            info!("Waiting several seconds before continuing...");
            thread::sleep(Duration::from_secs(6));
        }
    }

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

    let service = manager.create_service(&service_info, ServiceAccess::ALL_ACCESS)?;
    info!("Service '{}' created successfully.", SERVICE_NAME);
    Ok(service)
}