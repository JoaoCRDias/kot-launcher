use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Entrada de client dentro de um servidor
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClientEntry {
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub download_url: String,
    #[serde(default, rename = "preservedDirs")]
    pub preserved_dir: Vec<String>,
}

/// Entrada de servidor (production ou testServer) — cada um expõe os dois tipos de
/// client que coexistem: `cip` (client oficial da Cipsoft) e `otc` (KoliseuClient).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerEntry {
    #[serde(default)]
    pub cip: ClientEntry,
    #[serde(default)]
    pub otc: ClientEntry,
}

/// Manifest remoto completo retornado pela API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteManifest {
    pub production: ServerEntry,
    #[serde(rename = "testServer")]
    pub test_server: ServerEntry,
}

impl RemoteManifest {
    /// Obtém a entrada de client para um dado servidor e tipo (cip | otc)
    pub fn get_client(&self, server: &str, client_type: &str) -> Result<&ClientEntry, String> {
        let server_entry = match server {
            "production" => &self.production,
            "testServer" => &self.test_server,
            _ => return Err(format!("Servidor desconhecido: {}", server)),
        };
        match client_type {
            "cip" => Ok(&server_entry.cip),
            "otc" => Ok(&server_entry.otc),
            _ => Err(format!("Tipo de client desconhecido: {}", client_type)),
        }
    }
}

/// Entrada de ficheiro no manifest local (para verificação de integridade)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub hash: String,
    pub size: u64,
}

/// Manifest local armazenado no disco após instalação/atualização
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalManifest {
    pub version: String,
    /// Map de path relativo -> hash + size
    pub files: HashMap<String, FileEntry>,
}

impl LocalManifest {
    pub fn load(path: &Path) -> Option<Self> {
        let content = fs::read_to_string(path).ok()?;
        serde_json::from_str(&content).ok()
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        let content = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Erro ao serializar manifest: {}", e))?;
        fs::write(path, content)
            .map_err(|e| format!("Erro ao salvar manifest: {}", e))?;
        Ok(())
    }
}

/// Calcula o hash SHA-256 de um arquivo
pub fn calculate_file_hash(path: &Path) -> Result<String, String> {
    let data = fs::read(path)
        .map_err(|e| format!("Erro ao ler arquivo {}: {}", path.display(), e))?;
    let mut hasher = Sha256::new();
    hasher.update(&data);
    let result = hasher.finalize();
    Ok(hex::encode(result))
}

/// Resultado da verificação de integridade
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrityResult {
    pub total_files: usize,
    pub valid_files: usize,
    pub corrupted_files: Vec<String>,
    pub missing_files: Vec<String>,
}

/// Verifica a integridade dos arquivos instalados comparando hashes do manifest local
pub fn check_integrity(install_dir: &Path, manifest: &LocalManifest) -> IntegrityResult {
    let mut result = IntegrityResult {
        total_files: manifest.files.len(),
        valid_files: 0,
        corrupted_files: Vec::new(),
        missing_files: Vec::new(),
    };

    for (rel_path, entry) in &manifest.files {
        let file_path = install_dir.join(rel_path);
        if !file_path.exists() {
            result.missing_files.push(rel_path.clone());
            continue;
        }

        match calculate_file_hash(&file_path) {
            Ok(hash) => {
                if hash == entry.hash {
                    result.valid_files += 1;
                } else {
                    result.corrupted_files.push(rel_path.clone());
                }
            }
            Err(_) => {
                result.corrupted_files.push(rel_path.clone());
            }
        }
    }

    result
}

/// Gera hashes para todos os ficheiros num diretório (exceto preserve_paths e manifest.json)
pub fn generate_hashes(
    install_dir: &Path,
    preserve_paths: &[String],
) -> Result<HashMap<String, FileEntry>, String> {
    generate_hashes_with_progress(install_dir, preserve_paths, &|_, _| {})
}

/// Gera hashes com callback de progresso (current, total)
pub fn generate_hashes_with_progress<F>(
    install_dir: &Path,
    preserve_paths: &[String],
    on_progress: &F,
) -> Result<HashMap<String, FileEntry>, String>
where
    F: Fn(usize, usize),
{
    // Primeiro contar ficheiros
    let mut file_paths = Vec::new();
    collect_files(install_dir, install_dir, preserve_paths, &mut file_paths)?;

    let total = file_paths.len();
    let mut files = HashMap::new();

    for (i, (rel_path, path)) in file_paths.into_iter().enumerate() {
        let hash = calculate_file_hash(&path)?;
        let size = fs::metadata(&path)
            .map_err(|e| format!("Erro ao ler metadata: {}", e))?
            .len();
        files.insert(rel_path, FileEntry { hash, size });
        on_progress(i + 1, total);
    }

    Ok(files)
}

fn collect_files(
    base: &Path,
    dir: &Path,
    preserve_paths: &[String],
    out: &mut Vec<(String, std::path::PathBuf)>,
) -> Result<(), String> {
    let entries = fs::read_dir(dir)
        .map_err(|e| format!("Erro ao ler diretório {}: {}", dir.display(), e))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("Erro ao ler entrada: {}", e))?;
        let path = entry.path();
        let rel_path = path
            .strip_prefix(base)
            .map_err(|e| format!("Erro ao obter caminho relativo: {}", e))?
            .to_string_lossy()
            .replace('\\', "/");

        if rel_path == "manifest.json" {
            continue;
        }

        if preserve_paths.iter().any(|p| {
            let p_trimmed = p.trim_end_matches('/');
            rel_path == p_trimmed || rel_path.starts_with(&format!("{}/", p_trimmed))
        }) {
            continue;
        }

        if path.is_dir() {
            collect_files(base, &path, preserve_paths, out)?;
        } else {
            out.push((rel_path, path));
        }
    }

    Ok(())
}
