# Gerar e distribuir uma nova versão do Launcher

Runbook do release do **KoliseuOT Launcher** (Tauri v2 + NSIS + auto-updater
assinado). Cobre como gerar o `.exe` de distribuição, o `.sig` de assinatura, e
como publicar o update para que os launchers já instalados se atualizem sozinhos.

> O auto-update só aceita um instalador se a **assinatura** (`.sig`) bater com a
> **chave pública** embutida no launcher (`plugins.updater.pubkey` em
> `tauri.conf.json`). Sem isso, o update é rejeitado silenciosamente. Por isso
> **sempre** se builda com o script assinado — nunca com `tauri build` puro.

---

## 0. Pré-requisitos (uma vez)

- Node.js (>= 18) e Rust (toolchain MSVC `x86_64-pc-windows-msvc`)
- `npm install` na raiz do repo (instala o Tauri CLI e as deps)
- A **chave privada** de assinatura disponível no build (ver §6). Hoje ela está
  embutida no script `tauri:build` do `package.json` — leia o alerta de
  segurança da §6 antes de confiar nisso.

---

## 1. Subir a versão — nos **4** lugares

A versão do bundle (o número que vai no nome do `.exe` e que o updater compara)
vem do **`src-tauri/tauri.conf.json`**. Os outros arquivos precisam acompanhar
para não ficar inconsistente (npm, crate Rust e lockfile).

| Arquivo | Campo | Observação |
|---|---|---|
| `src-tauri/tauri.conf.json` | `"version"` | **fonte da verdade** do bundle/updater |
| `src-tauri/Cargo.toml` | `version = "..."` | versão do crate |
| `src-tauri/Cargo.lock` | bloco `name = "koliseuot-launcher"` → `version` | senão o build deixa o lockfile sujo |
| `package.json` | `"version"` | cosmético (não publicamos no npm), mas mantenha igual |

> Os 4 devem ter **exatamente o mesmo número**. Se o `tauri.conf.json` estiver
> atrás dos outros, o build gera um `.exe` com a versão antiga e o updater não
> oferece atualização.

Use [semver](https://semver.org/): `patch` (1.0.4 → 1.0.5) para correções,
`minor` (1.0 → 1.1) para features, `major` para quebras.

---

## 2. Buildar (assinado)

```bash
npm run tauri:build
```

O `tauri:build` chama `node scripts/sign-build.mjs`, que:

1. Lê a chave privada do arquivo **gitignored** `src-tauri/keys/koliseuot.key`
   (o conteúdo do arquivo JÁ é a string que o Tauri espera — o wrapper passa
   cru, sem re-encodar) e a injeta como `TAURI_SIGNING_PRIVATE_KEY`. **Nenhum
   segredo fica no `package.json`** (que é público).
2. Roda `tauri build`, que por sua vez:
   - executa `npm run build` (`tsc && vite build`) — o front;
   - compila o Rust em release;
   - gera o instalador **NSIS**;
   - como `bundle.createUpdaterArtifacts: true`, **assina** o instalador e
     gera o `.sig` ao lado.

> Para um release **transicional** durante uma rotação de chaves (assinar com a
> chave antiga, mas embutindo a pubkey nova), rode com override:
> `TAURI_KEY_FILE=src-tauri/keys/koliseuot-old.key node scripts/sign-build.mjs`.
> Nesse caso o Tauri emite um *warning* dizendo que a secret key não bate com a
> pubkey — **é esperado** quando se está rotacionando (ver §6).

> **NUNCA** rode só `tauri build` / `npm run tauri build` (sem passar pela
> chave) para um release: sem assinar, o `.sig` não é (re)gerado e fica defasado
> em relação ao novo `.exe`. Já aconteceu: um `.exe` 1.0.4 reconstruído com um
> `.sig` velho → updater rejeita.

> **Ambiente:** o `cargo`/Rust precisa estar no `PATH` (ex.: `~/.cargo/bin`).
> Em shells não-interativas pode não estar — exporte antes de buildar.

O build leva alguns minutos (compila o Rust do zero na primeira vez).

---

## 3. Artefatos gerados

```
src-tauri/target/release/bundle/nsis/
  KoliseuOT-Launcher_<versao>_x64-setup.exe       <- instalador + artefato de update
  KoliseuOT-Launcher_<versao>_x64-setup.exe.sig   <- assinatura (base64, ~432 bytes)
```

O `.exe` e o `.sig` são um **par casado**: o `.sig` é a assinatura *daquele*
`.exe` específico. Se rebuildar, os dois mudam juntos — sempre use o par da
mesma build.

Confira que o `.sig` foi gerado/atualizado agora (timestamp recente), não um
sobrando de release anterior.

---

## 4. Publicar o update

### 4.1 Hospedar o instalador

Suba o `KoliseuOT-Launcher_<versao>_x64-setup.exe` para o host de downloads
(o mesmo `url` que o endpoint vai apontar).

### 4.2 Atualizar o endpoint de update

O launcher consulta (config em `tauri.conf.json` → `plugins.updater.endpoints`):

```
GET https://koliseuot.com.br/api/launcher/updates/{{current_version}}
```

O `{{current_version}}` é substituído pela versão **instalada** de quem está
perguntando. O backend (koliseu-aac) decide:

- **Há update** → responde `200` com o JSON abaixo;
- **Já está atualizado** → responde `204 No Content` (sem body).

JSON de resposta (formato do updater do Tauri v2):

```json
{
  "version": "1.0.5",
  "notes": "Descrição das mudanças desta versão",
  "pub_date": "2026-06-21T00:00:00Z",
  "platforms": {
    "windows-x86_64": {
      "url": "https://koliseuot.com.br/downloads/launcher/KoliseuOT-Launcher_1.0.5_x64-setup.exe",
      "signature": "<conteúdo INTEGRAL do arquivo .sig>"
    }
  }
}
```

- `version`: a nova versão (igual ao `tauri.conf.json`).
- `url`: onde o `.exe` foi hospedado em 4.1.
- `signature`: **cole o conteúdo do arquivo `.sig`** (o base64 inteiro, não o
  caminho do arquivo).
- `pub_date`: ISO 8601 (UTC).

### 4.3 Pronto

Os launchers abertos detectam o update **ao abrir e a cada 15 minutos**
(ver `checkLauncherUpdate` em `src/App.tsx`). Baixam o `.exe`, validam o `.sig`
contra a `pubkey` embutida e reiniciam.

---

## 5. Verificar

1. Pegue uma máquina/instalação com a versão **anterior**.
2. Abra o launcher → deve aparecer o fluxo de update do próprio launcher.
3. Se nada acontece: confira (a) o endpoint está retornando `200` para a versão
   antiga; (b) a `version` do JSON é maior que a instalada; (c) o `signature`
   bate com o `.exe` hospedado (rebuild assinado resolve a maioria dos casos).

---

## 6. Chaves de assinatura

- **Privada (atual/nova)**: `src-tauri/keys/koliseuot.key` (gitignored —
  `src-tauri/keys/`). Lida pelo wrapper `scripts/sign-build.mjs` em tempo de
  build. **Não fica em nenhum arquivo versionado.**
- **Pública**: `src-tauri/keys/koliseuot.key.pub`; o valor também está embutido
  em `tauri.conf.json` → `plugins.updater.pubkey` (é o que vai dentro de cada
  launcher buildado).
- **Privada antiga (comprometida)**: `src-tauri/keys/koliseuot-old.key` — usada
  só para o release transicional 1.0.5. **Não usar para 1.0.6+.**
- Senha das chaves: **vazia**.
- Se perder a privada atual, **não dá** para assinar updates que os launchers já
  na pubkey nova aceitem (eles confiam só na pubkey embutida neles). Faça backup
  seguro de `koliseuot.key` fora do repo.

### Histórico: rotação feita em 1.0.5 (2026-06-21)

A chave antiga vivia **embutida em base64 no `package.json`** (repo público
`github.com/JoaoCRDias/kot-launcher`, senha vazia) → comprometida (qualquer um
podia extrair e assinar instaladores maliciosos aceitos pelo auto-updater).

Foi rotacionada assim:

1. Backup da antiga → `koliseuot-old.key`; nova gerada em `koliseuot.key`
   (`tauri signer generate -w src-tauri/keys/koliseuot.key -f`).
2. **Nova pubkey** no `tauri.conf.json`.
3. Segredo **tirado do `package.json`**: build passou a usar o wrapper
   `scripts/sign-build.mjs`, que lê a chave do arquivo gitignored.
4. **1.0.5 (transicional)**: buildado e assinado com a chave **antiga**
   (`TAURI_KEY_FILE=...koliseuot-old.key`), mas já embutindo a pubkey **nova**.
   Assim os launchers instalados (que confiam na pubkey antiga) aceitam o update
   e passam a ter a pubkey nova. Do **1.0.6 em diante**, `npm run tauri:build`
   assina com a chave nova (default), e a antiga é aposentada.

> A chave antiga ainda está no **histórico git público** (commits antigos do
> `package.json`). Como já foi rotacionada e só assina o 1.0.5, a urgência é
> baixa, mas para limpar de vez é preciso reescrever o histórico
> (git filter-repo / BFG) e dar `push --force` — operação destrutiva no repo
> público, faça com cuidado.

### Como rotacionar no futuro (sem o segredo no repo)

1. `npx tauri signer generate -w src-tauri/keys/koliseuot-novo.key -f -p ""`
2. Nova pubkey no `tauri.conf.json`.
3. Release transicional assinado com a chave **vigente** (a que os instalados
   confiam) embutindo a pubkey nova:
   `TAURI_KEY_FILE=src-tauri/keys/koliseuot.key node scripts/sign-build.mjs`
4. Promover a nova a default: renomear `koliseuot-novo.key` → `koliseuot.key`.
   A partir daí `npm run tauri:build` já assina com ela.

---

## Checklist rápido

- [ ] Bump de versão igual nos 4 arquivos (§1)
- [ ] `npm run tauri:build` (assinado, §2)
- [ ] `.exe` e `.sig` recém-gerados em `target/release/bundle/nsis/` (§3)
- [ ] `.exe` hospedado (§4.1)
- [ ] Endpoint atualizado: `version` + `url` + `signature` (conteúdo do `.sig`) + `pub_date` (§4.2)
- [ ] Testado a partir de uma instalação na versão anterior (§5)
