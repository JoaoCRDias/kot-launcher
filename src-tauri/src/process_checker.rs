use std::path::{Path, PathBuf};
use sysinfo::System;

/// Canonicaliza um caminho, com fallback para o próprio caminho se falhar
/// (ex.: a pasta ainda não existe). No Windows isso normaliza para o formato
/// `\\?\C:\...`, garantindo que `starts_with` compare prefixos consistentes.
fn canon(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

/// Retorna os caminhos dos executáveis EM EXECUÇÃO cujo binário está dentro de
/// `install_dir` — ou seja, exatamente o client que se quer atualizar/reparar,
/// e não qualquer `client.exe` aberto no sistema.
pub fn running_paths_in(install_dir: &Path) -> Vec<String> {
    let target = canon(install_dir);
    let sys = System::new_all();
    let mut found = Vec::new();

    for process in sys.processes().values() {
        if let Some(exe) = process.exe() {
            let exe_canon = canon(exe);
            if exe_canon.starts_with(&target) {
                let s = exe_canon.to_string_lossy().to_string();
                if !found.contains(&s) {
                    found.push(s);
                }
            }
        }
    }

    found
}

/// True se o client instalado em `install_dir` está rodando agora.
pub fn is_client_running_in(install_dir: &Path) -> bool {
    !running_paths_in(install_dir).is_empty()
}
