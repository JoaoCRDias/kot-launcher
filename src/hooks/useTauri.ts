import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export type Server = "production" | "testServer";
export type ClientType = "cip" | "otc";

export interface UpdateCheckResult {
  installed_version: string | null;
  remote_version: string;
  needs_update: boolean;
  download_available: boolean;
}

export interface IntegrityResult {
  total_files: number;
  valid_files: number;
  corrupted_files: string[];
  missing_files: string[];
}

export interface DownloadProgress {
  stage: string;
  bytes_downloaded: number;
  bytes_total: number;
  percentage: number;
}

export interface LauncherConfig {
  close_on_launch: boolean;
}

export async function checkClientRunning(
  server: Server,
  clientType: ClientType
): Promise<boolean> {
  return invoke<boolean>("check_client_running", { server, clientType });
}

export async function getRunningClients(
  server: Server,
  clientType: ClientType
): Promise<string[]> {
  return invoke<string[]>("get_running_clients", { server, clientType });
}

export async function getInstalledVersion(
  server: Server,
  clientType: ClientType
): Promise<string | null> {
  return invoke<string | null>("get_installed_version", { server, clientType });
}

export async function checkForUpdates(
  server: Server,
  clientType: ClientType
): Promise<UpdateCheckResult> {
  return invoke<UpdateCheckResult>("check_for_updates", { server, clientType });
}

export async function startUpdate(
  server: Server,
  clientType: ClientType
): Promise<string> {
  return invoke<string>("start_update", { server, clientType });
}

export async function verifyIntegrity(
  server: Server,
  clientType: ClientType
): Promise<IntegrityResult> {
  return invoke<IntegrityResult>("verify_integrity", { server, clientType });
}

export async function repairFiles(
  server: Server,
  clientType: ClientType
): Promise<IntegrityResult> {
  return invoke<IntegrityResult>("repair_files", { server, clientType });
}

export async function getInstallPath(
  server: Server,
  clientType: ClientType
): Promise<string> {
  return invoke<string>("get_install_path", { server, clientType });
}

export async function launchClient(
  server: Server,
  clientType: ClientType
): Promise<void> {
  return invoke<void>("launch_client", { server, clientType });
}

export function onUpdateProgress(
  callback: (progress: DownloadProgress) => void
) {
  return listen<DownloadProgress>("update-progress", (event) => {
    callback(event.payload);
  });
}

/** Persisted launcher preferences (%APPDATA%/KoliseuOT/launcher.json). */
export async function getLauncherConfig(): Promise<LauncherConfig> {
  return invoke<LauncherConfig>("get_launcher_config");
}

/** "Close on launch": exit the launcher automatically after the client starts. */
export async function setCloseOnLaunch(enabled: boolean): Promise<void> {
  return invoke<void>("set_close_on_launch", { enabled });
}
