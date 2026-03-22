import { getCurrentWindow } from "@tauri-apps/api/window";
import { getVersion } from "@tauri-apps/api/app";
import { useEffect, useState } from "react";

export default function TitleBar() {
  const appWindow = getCurrentWindow();
  const [version, setVersion] = useState("");

  useEffect(() => {
    getVersion().then((v) => setVersion(v));
  }, []);

  return (
    <div className="titlebar" data-tauri-drag-region>
      <span className="titlebar-title">KoliseuOT Launcher</span>
      <div className="titlebar-buttons">
        <span className="titlebar-version">v{version}</span>
        <button
          className="titlebar-btn minimize"
          onClick={() => appWindow.minimize()}
        >
          &#x2014;
        </button>
        <button
          className="titlebar-btn close"
          onClick={() => appWindow.close()}
        >
          &#x2715;
        </button>
      </div>
    </div>
  );
}
