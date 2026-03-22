use sysinfo::System;

const TIBIA_PROCESS_NAMES: &[&str] = &[
    "client.exe",
    "tibia.exe",
    "koliseuot.exe",
    "otclient.exe",
    "otclientv8.exe",
];

/// Verifica se algum processo do client Tibia está rodando
pub fn is_tibia_running() -> bool {
    let sys = System::new_all();
    for process in sys.processes().values() {
        let name = process.name().to_string_lossy().to_lowercase();
        for tibia_name in TIBIA_PROCESS_NAMES {
            if name == *tibia_name {
                return true;
            }
        }
    }
    false
}

/// Retorna lista de processos do Tibia encontrados
pub fn get_running_tibia_processes() -> Vec<String> {
    let sys = System::new_all();
    let mut found = Vec::new();
    for process in sys.processes().values() {
        let name = process.name().to_string_lossy().to_lowercase();
        for tibia_name in TIBIA_PROCESS_NAMES {
            if name == *tibia_name && !found.contains(&name) {
                found.push(name.clone());
            }
        }
    }
    found
}
