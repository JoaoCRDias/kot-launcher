import { DownloadProgress } from "../hooks/useTauri";

interface Props {
  progress: DownloadProgress | null;
}

const STAGE_LABELS: Record<string, string> = {
  checking: "Verificando...",
  download: "Baixando client...",
  cleaning: "Limpando ficheiros antigos...",
  extracting: "Extraindo ficheiros...",
  hashing: "Gerando verificação de integridade...",
  done: "Atualização concluída!",
};

function formatBytes(bytes: number): string {
  if (bytes === 0) return "0 B";
  const k = 1024;
  const sizes = ["B", "KB", "MB", "GB"];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + " " + sizes[i];
}

export default function ProgressBar({ progress }: Props) {
  if (!progress) return null;

  const percent = Math.round(progress.percentage);
  const label = STAGE_LABELS[progress.stage] || progress.stage;

  return (
    <div className="progress-container">
      <div className="progress-info">
        <span className="progress-text">{label}</span>
        {progress.stage === "download" && progress.bytes_total > 0 && (
          <span className="progress-stats">
            {formatBytes(progress.bytes_downloaded)} /{" "}
            {formatBytes(progress.bytes_total)}
          </span>
        )}
        {(progress.stage === "extracting" || progress.stage === "hashing") &&
          progress.bytes_total > 0 && (
          <span className="progress-stats">
            {progress.bytes_downloaded} / {progress.bytes_total} ficheiros
          </span>
        )}
      </div>
      <div className="progress-bar">
        <div className="progress-fill" style={{ width: `${percent}%` }} />
      </div>
      <span className="progress-percent">{percent}%</span>
    </div>
  );
}
