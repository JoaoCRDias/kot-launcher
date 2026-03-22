import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-shell";
import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import TitleBar from "./components/TitleBar";
import ProgressBar from "./components/ProgressBar";
import coinIcon from "./assets/coin.png";
import discordIcon from "./assets/discord.png";
import {
  checkTibiaRunning,
  checkForUpdates,
  startUpdate,
  verifyIntegrity,
  launchClient,
  repairFiles,
  getInstalledVersion,
  onUpdateProgress,
  DownloadProgress,
  UpdateCheckResult,
  Server,
  ClientType,
} from "./hooks/useTauri";

type LauncherStatus =
  | "checking"
  | "ready"
  | "update_available"
  | "updating"
  | "error"
  | "integrity_failed"
  | "verifying"
  | "repairing";

export default function App() {
  const [server, setServer] = useState<Server>("production");
  const [clientType, setClientType] = useState<ClientType>("cip");
  const [status, setStatus] = useState<LauncherStatus>("checking");
  const [version, setVersion] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [progress, setProgress] = useState<DownloadProgress | null>(null);
  const [statusMessage, setStatusMessage] = useState("Verificando atualizações...");
  const [downloadAvailable, setDownloadAvailable] = useState(true);

  // Discord URL - altere para o seu servidor
  const DISCORD_URL = "https://discord.gg/Qf3kRSt6Qd";
  const [launcherUpdating, setLauncherUpdating] = useState(false);
  const [launcherUpdateMsg, setLauncherUpdateMsg] = useState<string | null>(null);
  const [launcherUpdateProgress, setLauncherUpdateProgress] = useState(0);

  // Verificar update do próprio launcher ao abrir e a cada 15 minutos
  useEffect(() => {
    checkLauncherUpdate();
    const interval = setInterval(checkLauncherUpdate, 15 * 60 * 1000);
    return () => clearInterval(interval);
  }, []);

  async function checkLauncherUpdate() {
    try {
      if (status === "updating" || status === "repairing") return;

      setLauncherUpdateMsg("Verificando atualizações do launcher...");
      const update = await check();
      if (!update) {
        setLauncherUpdateMsg(null);
        return;
      }

      // Mostrar tela de update
      setLauncherUpdating(true);
      setLauncherUpdateMsg(`Nova versão v${update.version} encontrada. Preparando download...`);
      setLauncherUpdateProgress(0);

      // Pequeno delay para a UI renderizar
      await new Promise((r) => setTimeout(r, 500));

      let totalBytes = 0;
      let downloadedBytes = 0;

      setLauncherUpdateMsg(`Baixando v${update.version}...`);

      await update.download((event) => {
        switch (event.event) {
          case "Started":
            totalBytes = event.data.contentLength ?? 0;
            break;
          case "Progress":
            downloadedBytes += event.data.chunkLength;
            if (totalBytes > 0) {
              setLauncherUpdateProgress(Math.round((downloadedBytes / totalBytes) * 100));
            }
            break;
          case "Finished":
            setLauncherUpdateProgress(100);
            break;
        }
      });

      // Mostrar mensagem de instalação antes de fechar
      setLauncherUpdateMsg("Download concluído! Instalando... O launcher será reiniciado.");
      setLauncherUpdateProgress(100);

      // Delay para o usuário ler a mensagem
      await new Promise((r) => setTimeout(r, 2000));

      await update.install();
      await relaunch();
    } catch (err) {
      console.error("Erro ao atualizar launcher:", err);
      setLauncherUpdating(false);
      setLauncherUpdateMsg(null);
      setLauncherUpdateProgress(0);
    }
  }

  useEffect(() => {
    const unlisten = onUpdateProgress((p) => setProgress(p));
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  // Re-verificar quando mudar servidor ou tipo de client
  useEffect(() => {
    initCheck();
  }, [server, clientType]);

  async function initCheck() {
    try {
      setStatus("checking");
      setError(null);
      setStatusMessage("Verificando atualizações...");

      const installed = await getInstalledVersion(server, clientType);
      setVersion(installed);

      const result: UpdateCheckResult = await checkForUpdates(server, clientType);
      setDownloadAvailable(result.download_available);

      if (result.needs_update) {
        setStatus("update_available");
        if (!result.download_available) {
          setStatusMessage("Download não disponível para este client.");
        } else {
          setStatusMessage(
            installed
              ? `Nova versão disponível: ${result.remote_version}`
              : "Client não instalado. Clique para instalar."
          );
        }
      } else {
        setStatus("ready");
        setStatusMessage("Client atualizado e pronto!");
      }
    } catch (err) {
      setStatus("error");
      setError(String(err));
      setStatusMessage("Erro ao verificar atualizações");
    }
  }

  async function handleUpdate() {
    try {
      const tibiaRunning = await checkTibiaRunning();
      if (tibiaRunning) {
        setError("Feche o client do Tibia antes de atualizar!");
        return;
      }

      setStatus("updating");
      setError(null);
      setStatusMessage("Atualizando...");

      const newVersion = await startUpdate(server, clientType);
      setVersion(newVersion);
      setStatus("ready");
      setStatusMessage("Atualização concluída!");
      setProgress(null);
    } catch (err) {
      setStatus("error");
      setError(String(err));
      setStatusMessage("Erro na atualização");
    }
  }

  async function handleVerifyIntegrity() {
    try {
      setStatus("verifying");
      setStatusMessage("Verificando integridade dos arquivos...");

      const result = await verifyIntegrity(server, clientType);

      if (result.corrupted_files.length === 0 && result.missing_files.length === 0) {
        setStatusMessage(
          `Todos os ${result.total_files} arquivos estão íntegros!`
        );
        setStatus("ready");
      } else {
        const issues = result.corrupted_files.length + result.missing_files.length;
        setStatusMessage(
          `${issues} arquivo(s) com problema encontrado(s). Clique em "Reparar" para corrigir.`
        );
        setStatus("integrity_failed");
        setError(
          `Corrompidos: ${result.corrupted_files.length}, Faltando: ${result.missing_files.length}`
        );
      }
    } catch (err) {
      setStatus("error");
      setError(String(err));
      setStatusMessage("Erro ao verificar integridade");
    }
  }

  async function handleRepair() {
    try {
      const tibiaRunning = await checkTibiaRunning();
      if (tibiaRunning) {
        setError("Feche o client do Tibia antes de reparar!");
        return;
      }

      setStatus("repairing");
      setStatusMessage("Reparando arquivos...");

      const result = await repairFiles(server, clientType);
      setStatusMessage(
        `Reparo concluído! ${result.valid_files}/${result.total_files} arquivos OK.`
      );
      setStatus("ready");
      setError(null);
    } catch (err) {
      setStatus("error");
      setError(String(err));
      setStatusMessage("Erro ao reparar arquivos");
    }
  }

  const [launching, setLaunching] = useState(false);

  async function handlePlay() {
    try {
      setLaunching(true);
      setError(null);
      await launchClient(server, clientType);
    } catch (err) {
      setError(String(err));
    } finally {
      setLaunching(false);
    }
  }

  function handleDiscord() {
    open(DISCORD_URL);
  }

  function handleSite() {
    open("https://koliseuot.com.br/");
  }

  function handleDonate() {
    open("https://koliseuot.com.br/donate");
  }

  const isLoading =
    status === "checking" || status === "updating" || status === "verifying" || status === "repairing";

  const serverLabel = server === "production" ? "Production" : "Test Server";
  const clientLabel = clientType === "cip" ? "CIP Client" : "OTC Client";

  if (launcherUpdating) {
    return (
      <div className="launcher">
        <TitleBar />
        <div className="launcher-content">
          <div className="launcher-header">
            <h1 className="launcher-logo">KoliseuOT</h1>
            <p className="launcher-subtitle">{launcherUpdateMsg}</p>
          </div>
          <div className="status-area">
            <div className="progress-container">
              <div className="progress-bar">
                <div className="progress-fill" style={{ width: `${launcherUpdateProgress}%` }} />
              </div>
              <span className="progress-percent">{launcherUpdateProgress}%</span>
            </div>
            <button className="btn btn-loading" disabled>
              ATUALIZANDO LAUNCHER...
            </button>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="launcher">
      <TitleBar />

      <div className="launcher-content">
        {/* Header / Logo area */}
        <div className="launcher-header">
          <h1 className="launcher-logo">KoliseuOT</h1>
          <p className="launcher-subtitle">
            {version ? `v${version}` : "Sem instalação"} — {serverLabel} / {clientLabel}
          </p>
        </div>

        {/* Server & Client tabs */}
        <div className="tabs-container">
          <div className="tab-group">
            <span className="tab-group-label">Servidor</span>
            <button
              className={`tab-btn ${server === "production" ? "active" : ""}`}
              onClick={() => setServer("production")}
              disabled={isLoading}
            >
              Production
            </button>
            <button
              className={`tab-btn ${server === "testServer" ? "active" : ""}`}
              onClick={() => setServer("testServer")}
              disabled={isLoading}
            >
              Test Server
            </button>
          </div>
          <div className="tab-group">
            <span className="tab-group-label">Client</span>
            <button
              className={`tab-btn ${clientType === "cip" ? "active" : ""}`}
              onClick={() => setClientType("cip")}
              disabled={isLoading}
            >
              CIP
            </button>
            <button
              className={`tab-btn ${clientType === "otc" ? "active" : ""}`}
              onClick={() => setClientType("otc")}
              disabled={isLoading}
            >
              OTC
            </button>
          </div>
        </div>

        {/* Status area */}
        <div className="status-area">
          <p className={`status-message status-${status}`}>{statusMessage}</p>
          {error && <p className="error-message">{error}</p>}
          {status === "updating" && <ProgressBar progress={progress} />}
        </div>

        {/* Actions */}
        <div className="launcher-actions">
          {status === "ready" && (
            <button className="btn btn-play" onClick={handlePlay} disabled={launching}>
              {launching ? "INICIANDO..." : "JOGAR"}
            </button>
          )}

          {status === "update_available" && (
            <button
              className="btn btn-update"
              onClick={handleUpdate}
              disabled={!downloadAvailable}
              title={!downloadAvailable ? "Download não disponível para este client" : ""}
            >
              {version ? "ATUALIZAR" : "INSTALAR"}
            </button>
          )}

          {(status === "checking" || status === "updating" || status === "verifying" || status === "repairing") && (
            <button className="btn btn-loading" disabled>
              {status === "updating" ? "ATUALIZANDO..." :
                status === "verifying" ? "VERIFICANDO..." :
                  status === "repairing" ? "REPARANDO..." :
                    "VERIFICANDO..."}
            </button>
          )}

          {status === "integrity_failed" && (
            <button className="btn btn-repair" onClick={handleRepair}>
              REPARAR
            </button>
          )}

          {status === "error" && (
            <>
              <button className="btn btn-repair" onClick={handleRepair}>
                REPARAR
              </button>
              <button className="btn btn-retry" onClick={initCheck}>
                TENTAR NOVAMENTE
              </button>
            </>
          )}
        </div>

        {/* Bottom bar */}
        <div className="launcher-footer">
          <div className="footer-left">
            <button
              className="btn-icon"
              onClick={handleVerifyIntegrity}
              disabled={isLoading}
              title="Verificar integridade"
            >
              &#x2714; Verificar Arquivos
            </button>
          </div>
          <div className="footer-right">
            <button className="btn-footer" onClick={handleDonate} title="Doar">
              <img src={coinIcon} alt="coin" className="btn-icon-img" /> Donate
            </button>
            <button className="btn-footer" onClick={handleSite} title="Site">
              Site
            </button>
            <button className="btn-discord" onClick={handleDiscord}>
              <img src={discordIcon} alt="discord" className="btn-icon-img" /> Discord
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
