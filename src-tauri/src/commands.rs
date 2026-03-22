use std::sync::Arc;

use crate::manifest::IntegrityResult;
use crate::process_checker;
use crate::updater;
use tauri::{Emitter, Window};

/// URL da API de verificação de versão
const MANIFEST_URL: &str = "https://koliseuot.com.br/api/client/version";

#[tauri::command]
pub fn check_tibia_running() -> bool {
    process_checker::is_tibia_running()
}

#[tauri::command]
pub fn get_running_tibia_processes() -> Vec<String> {
    process_checker::get_running_tibia_processes()
}

#[tauri::command]
pub fn get_installed_version(server: String, client_type: String) -> Option<String> {
    updater::get_installed_version(&server, &client_type)
}

#[tauri::command]
pub async fn check_for_updates(
    server: String,
    client_type: String,
) -> Result<UpdateCheckResult, String> {
    let remote = updater::fetch_remote_manifest(MANIFEST_URL).await?;
    let client_entry = remote.get_client(&server, &client_type)?;
    let installed = updater::get_installed_version(&server, &client_type);

    let needs_update = match &installed {
        Some(v) => v != &client_entry.version,
        None => true,
    };

    Ok(UpdateCheckResult {
        installed_version: installed,
        remote_version: client_entry.version.clone(),
        needs_update,
        download_available: !client_entry.download_url.is_empty(),
    })
}

#[derive(serde::Serialize)]
pub struct UpdateCheckResult {
    pub installed_version: Option<String>,
    pub remote_version: String,
    pub needs_update: bool,
    pub download_available: bool,
}

#[tauri::command]
pub async fn start_update(
    window: Window,
    server: String,
    client_type: String,
) -> Result<String, String> {
    if process_checker::is_tibia_running() {
        return Err("Feche o client do Tibia antes de atualizar.".to_string());
    }

    let win = window.clone();
    let version =
        updater::perform_update(MANIFEST_URL, &server, &client_type, move |progress| {
            let _ = win.emit("update-progress", &progress);
        })
        .await?;

    Ok(version)
}

#[tauri::command]
pub fn verify_integrity(
    server: String,
    client_type: String,
) -> Result<IntegrityResult, String> {
    updater::verify_integrity(&server, &client_type)
}

#[tauri::command]
pub async fn repair_files(
    server: String,
    client_type: String,
) -> Result<IntegrityResult, String> {
    if process_checker::is_tibia_running() {
        return Err("Feche o client do Tibia antes de reparar.".to_string());
    }

    updater::repair_files(MANIFEST_URL, &server, &client_type).await
}

#[tauri::command]
pub fn get_install_path(server: String, client_type: String) -> String {
    updater::get_install_dir(&server, &client_type)
        .to_string_lossy()
        .to_string()
}

#[tauri::command]
pub async fn launch_client(server: String, client_type: String) -> Result<(), String> {
    let install_dir = updater::get_install_dir(&server, &client_type);

    let (exe_path, work_dir) = if client_type == "otc" {
        (install_dir.join("otclient.exe"), install_dir.clone())
    } else {
        (install_dir.join("bin").join("client.exe"), install_dir.join("bin"))
    };

    if !exe_path.exists() {
        return Err("Executável do client não encontrado. Verifique a instalação.".to_string());
    }

    let child = std::process::Command::new(&exe_path)
        .current_dir(&work_dir)
        .spawn()
        .map_err(|e| format!("Erro ao iniciar client: {}", e))?;

    let pid = child.id();

    // Esperar até o processo criar uma janela visível (max 30s)
    tokio::task::spawn_blocking(move || {
        wait_for_window(pid, std::time::Duration::from_secs(30))
    })
    .await
    .map_err(|e| format!("Erro ao aguardar janela: {}", e))?;

    Ok(())
}

/// Aguarda o processo com o PID fornecido criar uma janela visível
fn wait_for_window(pid: u32, timeout: std::time::Duration) {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    let start = std::time::Instant::now();

    while start.elapsed() < timeout {
        let found = Arc::new(AtomicBool::new(false));
        let found_clone = found.clone();
        let target_pid = pid;

        unsafe {
            winapi::um::winuser::EnumWindows(
                Some(enum_windows_callback),
                &(target_pid, found_clone) as *const (u32, Arc<AtomicBool>) as isize,
            );
        }

        if found.load(Ordering::Relaxed) {
            return;
        }

        std::thread::sleep(std::time::Duration::from_millis(250));
    }
}

unsafe extern "system" fn enum_windows_callback(
    hwnd: winapi::shared::windef::HWND,
    lparam: isize,
) -> i32 {
    let data = &*(lparam as *const (u32, Arc<std::sync::atomic::AtomicBool>));
    let (target_pid, found) = data;

    let mut process_id: u32 = 0;
    winapi::um::winuser::GetWindowThreadProcessId(hwnd, &mut process_id);

    if process_id == *target_pid && winapi::um::winuser::IsWindowVisible(hwnd) != 0 {
        found.store(true, std::sync::atomic::Ordering::Relaxed);
        return 0; // parar enumeração
    }

    1 // continuar enumeração
}
