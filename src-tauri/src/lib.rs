mod commands;
mod manifest;
mod process_checker;
mod updater;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            app.handle().plugin(
                tauri_plugin_log::Builder::default()
                    .level(log::LevelFilter::Info)
                    .build(),
            )?;

            // Inicializar plugin de auto-update do launcher
            app.handle().plugin(tauri_plugin_updater::Builder::new().build())?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::check_client_running,
            commands::get_running_clients,
            commands::get_installed_version,
            commands::check_for_updates,
            commands::start_update,
            commands::verify_integrity,
            commands::repair_files,
            commands::get_install_path,
            commands::launch_client,
            commands::get_launcher_config,
            commands::set_close_on_launch,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
