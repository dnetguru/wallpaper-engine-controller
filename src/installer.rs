use std::fs;
use std::env;
use std::process::exit;
use std::path::{Path, PathBuf};
use std::ffi::{OsStr, OsString};
use tracing::{debug, error, info};

use nameof::name_of;
use clap::CommandFactory;

use windows_elevate::{check_elevated, elevate};
use windows_service::{
    service::{
        ServiceAccess, ServiceErrorControl, ServiceStartType, ServiceType,
    },
    service_manager::{ServiceManager, ServiceManagerAccess},
};
use windows_service::service::{Service, ServiceInfo};

use crate::Cli;

const SERVICE_NAME: &str = "WallpaperControllerService";
const SERVICE_DISPLAY_NAME: &str = "Wallpaper Controller Service";

pub fn handle_installation(args: &Cli) {
    if args.install.is_none() && !args.add_startup_service {
        return; // Nothing to do
    }

    if !check_elevated().unwrap_or(false) {
        info!("Requesting administrator privileges...");
        elevate().expect("Failed to elevate administrator privileges");
        exit(0); // Exit the non-elevated process
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
                error!("Installation failed: {:}", e);
                exit(1);
            }
        }
    }

    if args.add_startup_service {
        let mut service_args = Vec::new();
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
                    service_args.push(arg.into());
                }
            }
        }

        match setup_startup_service(&exe_path, service_args) {
            Ok(svc) => {
                info!("Successfully set up the startup service.");
                if let Err(e) = svc.start::<&str>(&[]) {
                    error!("Failed to start the startup service: {:?}", e);
                } else {
                    info!("Successfully started the service.");
                }
            },
            Err(e) => {
                error!("Failed to set up startup service: {:?}", e);
                exit(1);
            }
        }
    }

    info!("Operations completed successfully.");
    exit(0);
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

    if let Ok(service) = manager.open_service(SERVICE_NAME, ServiceAccess::all()) {
        info!("Service '{}' already exists. Deleting it.", SERVICE_NAME);
        if let Err(e) = service.delete() {
            error!("Failed to delete service '{}'. You might need to log out and/or restart your computer to proceed", SERVICE_NAME);
            error!("Error: {}", e);
            exit(2);
        }
    }

    debug!("Executable: {} | Launch args: {:?}", SERVICE_NAME, launch_args);

    let service_info = ServiceInfo {
        name: SERVICE_NAME.into(),
        display_name: SERVICE_DISPLAY_NAME.into(),
        service_type: ServiceType::OWN_PROCESS,
        start_type: ServiceStartType::AutoStart,
        error_control: ServiceErrorControl::Normal,
        executable_path: PathBuf::from(exe_path),
        launch_arguments: launch_args,
        dependencies: vec![],
        account_name: None,
        account_password: None,
    };

    let service = manager.create_service(&service_info, ServiceAccess::ALL_ACCESS)?;
    info!("Service '{}' created successfully.", SERVICE_NAME);
    Ok(service)
}