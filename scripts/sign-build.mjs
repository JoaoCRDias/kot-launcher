// Builda o app Tauri e assina os artefatos de update.
//
// Lê a chave privada de um arquivo GITIGNORED (nunca versionado) e a injeta no
// bundler via TAURI_SIGNING_PRIVATE_KEY. O conteúdo do arquivo de chave JÁ é a
// string que o Tauri espera (não re-encodar em base64 — isso gera base64 duplo
// e quebra com "Missing encoded key"). O Tauri não honra
// TAURI_SIGNING_PRIVATE_KEY_PATH neste fluxo — precisa do conteúdo. Manter a
// chave em arquivo gitignored evita ter o segredo no package.json (público).
//
// Chave padrão: src-tauri/keys/koliseuot.key (a chave atual/nova).
// Override:     TAURI_KEY_FILE=<caminho>  (ex.: a chave antiga, p/ um release
//               transicional durante a rotação de chaves).
//
// Uso: node scripts/sign-build.mjs   (ou: npm run tauri:build)

import { readFileSync, existsSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const keyFile = resolve(root, process.env.TAURI_KEY_FILE || "src-tauri/keys/koliseuot.key");

if (!existsSync(keyFile)) {
  console.error(`[sign-build] chave de assinatura nao encontrada: ${keyFile}`);
  console.error("[sign-build] gere uma com: npx tauri signer generate -w src-tauri/keys/koliseuot.key");
  process.exit(1);
}

const key = readFileSync(keyFile, "utf8").trim();

const env = {
  ...process.env,
  TAURI_SIGNING_PRIVATE_KEY: key,
  TAURI_SIGNING_PRIVATE_KEY_PASSWORD: process.env.TAURI_SIGNING_PRIVATE_KEY_PASSWORD ?? "",
};

console.log(`[sign-build] assinando com: ${keyFile}`);

const tauriCli = resolve(root, "node_modules/@tauri-apps/cli/tauri.js");
const res = spawnSync(process.execPath, [tauriCli, "build", ...process.argv.slice(2)], {
  stdio: "inherit",
  env,
  cwd: root,
});

process.exit(res.status ?? 1);
