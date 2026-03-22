use crate::manifest::{
    check_integrity, generate_hashes, generate_hashes_with_progress, IntegrityResult,
    LocalManifest, RemoteManifest,
};
use reqwest::Client;
use std::fs;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use zip::ZipArchive;

/// Estado de progresso do download
#[derive(Debug, Clone, serde::Serialize)]
pub struct DownloadProgress {
    pub stage: String,
    pub bytes_downloaded: u64,
    pub bytes_total: u64,
    pub percentage: f64,
}

/// Diretório base de instalação
fn get_base_dir() -> PathBuf {
    let app_data = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(app_data).join("KoliseuOT")
}

/// Diretório de instalação para um servidor/client específico
/// Ex: %APPDATA%/KoliseuOT/production/cip
pub fn get_install_dir(server: &str, client_type: &str) -> PathBuf {
    get_base_dir().join(server).join(client_type)
}

/// Caminho do manifest local para um servidor/client específico
pub fn get_local_manifest_path(server: &str, client_type: &str) -> PathBuf {
    get_install_dir(server, client_type).join("manifest.json")
}

/// Busca o manifest remoto do servidor
pub async fn fetch_remote_manifest(url: &str) -> Result<RemoteManifest, String> {
    let client = Client::new();
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Erro ao conectar ao servidor: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("Servidor retornou status: {}", response.status()));
    }

    let manifest: RemoteManifest = response
        .json()
        .await
        .map_err(|e| format!("Erro ao parsear manifest: {}", e))?;

    Ok(manifest)
}

/// Remove ficheiros que não estão na lista de preservados
fn cleanup_files(install_dir: &Path, preserve_paths: &[String]) -> Result<(), String> {
    if !install_dir.exists() {
        return Ok(());
    }
    cleanup_dir_recursive(install_dir, install_dir, preserve_paths)
}

fn cleanup_dir_recursive(
    base: &Path,
    dir: &Path,
    preserve_paths: &[String],
) -> Result<(), String> {
    let entries = fs::read_dir(dir)
        .map_err(|e| format!("Erro ao ler diretório {}: {}", dir.display(), e))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("Erro ao ler entrada: {}", e))?;
        let path = entry.path();
        let rel_path = path
            .strip_prefix(base)
            .map_err(|e| format!("Erro: {}", e))?
            .to_string_lossy()
            .replace('\\', "/");

        // Preservar manifest.json
        if rel_path == "manifest.json" {
            continue;
        }

        // Verificar se está nos preserve_paths
        if preserve_paths.iter().any(|p| rel_path.starts_with(p)) {
            continue;
        }

        if path.is_dir() {
            cleanup_dir_recursive(base, &path, preserve_paths)?;
            // Remover diretório se ficou vazio
            if fs::read_dir(&path).map(|mut d| d.next().is_none()).unwrap_or(false) {
                let _ = fs::remove_dir(&path);
            }
        } else {
            fs::remove_file(&path)
                .map_err(|e| format!("Erro ao remover {}: {}", rel_path, e))?;
        }
    }

    Ok(())
}

/// Baixa o ZIP e retorna os bytes, emitindo progresso por chunk
async fn download_zip<F>(client: &Client, url: &str, on_progress: &F) -> Result<Vec<u8>, String>
where
    F: Fn(DownloadProgress),
{
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Erro ao baixar ZIP: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("Erro ao baixar ZIP: status {}", response.status()));
    }

    let total_size = response.content_length().unwrap_or(0);
    let mut downloaded: u64 = 0;
    let mut buffer = Vec::with_capacity(total_size as usize);

    let mut stream = response;
    while let Some(chunk) = stream
        .chunk()
        .await
        .map_err(|e| format!("Erro ao ler chunk: {}", e))?
    {
        downloaded += chunk.len() as u64;
        buffer.extend_from_slice(&chunk);

        on_progress(DownloadProgress {
            stage: "download".to_string(),
            bytes_downloaded: downloaded,
            bytes_total: total_size,
            percentage: if total_size > 0 {
                (downloaded as f64 / total_size as f64) * 100.0
            } else {
                0.0
            },
        });
    }

    Ok(buffer)
}

/// Extrai um ZIP para o diretório de instalação, emitindo progresso por ficheiro
fn extract_zip<F>(zip_data: &[u8], install_dir: &Path, on_progress: &F) -> Result<(), String>
where
    F: Fn(DownloadProgress),
{
    let cursor = Cursor::new(zip_data);
    let mut archive =
        ZipArchive::new(cursor).map_err(|e| format!("Erro ao abrir ZIP: {}", e))?;

    // Detectar prefixo comum (GitHub ZIPs geralmente têm uma pasta raiz tipo "repo-v1.0.0/")
    let prefix = detect_zip_prefix(&mut archive);
    let total = archive.len();

    for i in 0..total {
        let mut file = archive
            .by_index(i)
            .map_err(|e| format!("Erro ao ler entrada do ZIP: {}", e))?;

        let raw_name = file.name().to_string();

        // Remover prefixo se existir
        let rel_path = if let Some(ref pfx) = prefix {
            match raw_name.strip_prefix(pfx) {
                Some(stripped) => stripped.to_string(),
                None => continue,
            }
        } else {
            raw_name.clone()
        };

        // Ignorar entradas vazias
        if rel_path.is_empty() {
            continue;
        }

        let dest = install_dir.join(&rel_path);

        if file.is_dir() {
            fs::create_dir_all(&dest)
                .map_err(|e| format!("Erro ao criar diretório {}: {}", rel_path, e))?;
        } else {
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("Erro ao criar diretório: {}", e))?;
            }
            let mut content = Vec::new();
            file.read_to_end(&mut content)
                .map_err(|e| format!("Erro ao ler ficheiro do ZIP {}: {}", rel_path, e))?;
            fs::write(&dest, &content)
                .map_err(|e| format!("Erro ao escrever {}: {}", rel_path, e))?;
        }

        on_progress(DownloadProgress {
            stage: "extracting".to_string(),
            bytes_downloaded: (i + 1) as u64,
            bytes_total: total as u64,
            percentage: ((i + 1) as f64 / total as f64) * 100.0,
        });
    }

    Ok(())
}

/// Detecta o prefixo comum em ZIPs do GitHub (ex: "repo-name-v1.0.0/")
fn detect_zip_prefix(archive: &mut ZipArchive<Cursor<&[u8]>>) -> Option<String> {
    if archive.len() == 0 {
        return None;
    }

    let first_name = archive.by_index(0).ok()?.name().to_string();

    if first_name.ends_with('/') {
        let all_match = (0..archive.len()).all(|i| {
            archive
                .by_index(i)
                .map(|f| f.name().starts_with(&first_name))
                .unwrap_or(false)
        });
        if all_match {
            return Some(first_name);
        }
    }

    None
}

/// Executa a atualização completa de um client específico
pub async fn perform_update<F>(
    manifest_url: &str,
    server: &str,
    client_type: &str,
    on_progress: F,
) -> Result<String, String>
where
    F: Fn(DownloadProgress) + Send + 'static,
{
    let install_dir = get_install_dir(server, client_type);
    fs::create_dir_all(&install_dir)
        .map_err(|e| format!("Erro ao criar diretório de instalação: {}", e))?;

    // 1. Buscar manifest remoto
    on_progress(DownloadProgress {
        stage: "checking".to_string(),
        bytes_downloaded: 0,
        bytes_total: 0,
        percentage: 0.0,
    });
    let remote = fetch_remote_manifest(manifest_url).await?;
    let client_entry = remote.get_client(server, client_type)?;

    // 2. Baixar ZIP do GitHub
    on_progress(DownloadProgress {
        stage: "download".to_string(),
        bytes_downloaded: 0,
        bytes_total: 0,
        percentage: 0.0,
    });
    let http_client = Client::new();
    let zip_data = download_zip(&http_client, &client_entry.download_url, &on_progress).await?;

    let preserve = &client_entry.preserved_dir;

    // 3. Limpar ficheiros antigos (preservando os configurados)
    on_progress(DownloadProgress {
        stage: "cleaning".to_string(),
        bytes_downloaded: 0,
        bytes_total: 0,
        percentage: 50.0,
    });
    cleanup_files(&install_dir, preserve)?;

    // 4. Extrair ZIP
    extract_zip(&zip_data, &install_dir, &on_progress)?;

    // 5. Gerar hashes dos ficheiros extraídos (para verificação de integridade futura)
    let files = generate_hashes_with_progress(&install_dir, preserve, &|current, total| {
        on_progress(DownloadProgress {
            stage: "hashing".to_string(),
            bytes_downloaded: current as u64,
            bytes_total: total as u64,
            percentage: (current as f64 / total as f64) * 100.0,
        });
    })?;

    // 6. Salvar manifest local
    let local = LocalManifest {
        version: client_entry.version.clone(),
        files,
    };
    local.save(&get_local_manifest_path(server, client_type))?;

    on_progress(DownloadProgress {
        stage: "done".to_string(),
        bytes_downloaded: 0,
        bytes_total: 0,
        percentage: 100.0,
    });

    Ok(client_entry.version.clone())
}

/// Verifica integridade dos arquivos instalados de um client específico
pub fn verify_integrity(server: &str, client_type: &str) -> Result<IntegrityResult, String> {
    let install_dir = get_install_dir(server, client_type);
    let manifest_path = get_local_manifest_path(server, client_type);

    let local = LocalManifest::load(&manifest_path)
        .ok_or("Nenhuma instalação encontrada. Faça o download primeiro.")?;

    Ok(check_integrity(&install_dir, &local))
}

/// Repara arquivos corrompidos — re-baixa o ZIP e extrai só os ficheiros com problema
pub async fn repair_files(
    manifest_url: &str,
    server: &str,
    client_type: &str,
) -> Result<IntegrityResult, String> {
    let install_dir = get_install_dir(server, client_type);
    let manifest_path = get_local_manifest_path(server, client_type);

    let local = LocalManifest::load(&manifest_path)
        .ok_or("Nenhuma instalação encontrada.")?;

    let integrity = check_integrity(&install_dir, &local);

    if integrity.corrupted_files.is_empty() && integrity.missing_files.is_empty() {
        return Ok(integrity);
    }

    // Precisa re-baixar o ZIP para reparar
    let remote = fetch_remote_manifest(manifest_url).await?;
    let client_entry = remote.get_client(server, client_type)?;

    let http_client = Client::new();
    let zip_data = download_zip(&http_client, &client_entry.download_url, &|_| {}).await?;

    let cursor = Cursor::new(zip_data.as_slice());
    let mut archive =
        ZipArchive::new(cursor).map_err(|e| format!("Erro ao abrir ZIP: {}", e))?;

    let prefix = detect_zip_prefix_from_slice(&zip_data);

    let files_to_repair: Vec<String> = integrity
        .corrupted_files
        .iter()
        .chain(integrity.missing_files.iter())
        .cloned()
        .collect();

    // Extrair apenas os ficheiros com problema
    for i in 0..archive.len() {
        let mut file = archive.by_index(i).map_err(|e| format!("Erro: {}", e))?;
        let raw_name = file.name().to_string();

        let rel_path = if let Some(ref pfx) = prefix {
            match raw_name.strip_prefix(pfx) {
                Some(stripped) => stripped.to_string(),
                None => continue,
            }
        } else {
            raw_name.clone()
        };

        if rel_path.is_empty() || file.is_dir() {
            continue;
        }

        // Só extrair ficheiros que precisam de reparo
        if !files_to_repair.iter().any(|f| *f == rel_path) {
            continue;
        }

        let dest = install_dir.join(&rel_path);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).ok();
        }
        let mut content = Vec::new();
        file.read_to_end(&mut content)
            .map_err(|e| format!("Erro ao ler {}: {}", rel_path, e))?;
        fs::write(&dest, &content)
            .map_err(|e| format!("Erro ao escrever {}: {}", rel_path, e))?;
    }

    // Regenerar hashes e salvar manifest
    let preserve = &client_entry.preserved_dir;
    let files = generate_hashes(&install_dir, preserve)?;
    let new_local = LocalManifest {
        version: local.version.clone(),
        files,
    };
    new_local.save(&manifest_path)?;

    Ok(check_integrity(&install_dir, &new_local))
}

fn detect_zip_prefix_from_slice(data: &[u8]) -> Option<String> {
    let cursor = Cursor::new(data);
    let mut archive = ZipArchive::new(cursor).ok()?;

    if archive.len() == 0 {
        return None;
    }

    let first_name = archive.by_index(0).ok()?.name().to_string();

    if first_name.ends_with('/') {
        let all_match = (0..archive.len()).all(|i| {
            archive
                .by_index(i)
                .map(|f| f.name().starts_with(&first_name))
                .unwrap_or(false)
        });
        if all_match {
            return Some(first_name);
        }
    }

    None
}

/// Retorna informações sobre a versão instalada de um client específico
pub fn get_installed_version(server: &str, client_type: &str) -> Option<String> {
    let manifest_path = get_local_manifest_path(server, client_type);
    let local = LocalManifest::load(&manifest_path)?;
    Some(local.version)
}

