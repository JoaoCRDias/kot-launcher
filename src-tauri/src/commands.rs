use std::sync::Arc;

use crate::manifest::IntegrityResult;
use crate::process_checker;
use crate::updater;
use tauri::{Emitter, Window};

/// URL da API de verificação de versão
const MANIFEST_URL: &str = "https://koliseuot.com.br/api/client/version";

// ---------------------------------------------------------------------------
// Configuração persistente do launcher (%APPDATA%/KoliseuOT/launcher.json)
// ---------------------------------------------------------------------------
#[derive(serde::Serialize, serde::Deserialize, Default)]
pub struct LauncherConfig {
    /// Se true, o launcher fecha automaticamente após iniciar o client.
    #[serde(default)]
    close_on_launch: bool,
}

fn launcher_config_path() -> std::path::PathBuf {
    let app_data = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
    std::path::PathBuf::from(app_data)
        .join("KoliseuOT")
        .join("launcher.json")
}

fn read_launcher_config() -> LauncherConfig {
    std::fs::read_to_string(launcher_config_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

#[tauri::command]
pub fn get_launcher_config() -> LauncherConfig {
    read_launcher_config()
}

#[tauri::command]
pub fn set_close_on_launch(enabled: bool) -> Result<(), String> {
    let path = launcher_config_path();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let cfg = LauncherConfig {
        close_on_launch: enabled,
    };
    let json = serde_json::to_string_pretty(&cfg).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn check_client_running(server: String, client_type: String) -> bool {
    let dir = updater::get_install_dir(&server, &client_type);
    process_checker::is_client_running_in(&dir)
}

#[tauri::command]
pub fn get_running_clients(server: String, client_type: String) -> Vec<String> {
    let dir = updater::get_install_dir(&server, &client_type);
    process_checker::running_paths_in(&dir)
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
    // Bloqueia só se ESTE client (o que está a ser atualizado) estiver aberto,
    // não qualquer client.exe do sistema.
    let install_dir = updater::get_install_dir(&server, &client_type);
    if process_checker::is_client_running_in(&install_dir) {
        return Err("Feche este client (a versão que está a atualizar) antes de continuar.".to_string());
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
    let install_dir = updater::get_install_dir(&server, &client_type);
    if process_checker::is_client_running_in(&install_dir) {
        return Err("Feche este client (a versão que está a reparar) antes de continuar.".to_string());
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

    // CIP (client oficial da Cipsoft) roda bin/client.exe; OTC (KoliseuClient) roda o
    // executável na raiz da pasta instalada.
    let (exe_path, work_dir) = if client_type == "otc" {
        (install_dir.join("KoliseuClient.exe"), install_dir.clone())
    } else {
        (install_dir.join("bin").join("client.exe"), install_dir.join("bin"))
    };

    if !exe_path.exists() {
        return Err("Executável do client não encontrado. Verifique a instalação.".to_string());
    }

    // Passa o caminho do próprio launcher para o client, para que o botão
    // "Change Client" (entergame.lua -> os.getenv("KOLISEU_LAUNCHER")) consiga
    // reabrir este launcher independentemente do layout de instalação.
    let launcher_path = std::env::current_exe()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    let mut command = std::process::Command::new(&exe_path);
    command.current_dir(&work_dir);
    command.env("KOLISEU_LAUNCHER", launcher_path);

    // OTC-only: informa ao client a versão instalada + o ambiente, para o gate de
    // atualização no login (entergame.lua compara com /api/client/version e bloqueia
    // o login se estiver desatualizado). O client oficial (CIP) ignora estas vars.
    if client_type == "otc" {
        let installed = updater::get_installed_version(&server, &client_type).unwrap_or_default();
        command.env("KOLISEU_CLIENT_VERSION", installed);
        command.env("KOLISEU_CLIENT_ENV", &server);
    }

    let child = command
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
