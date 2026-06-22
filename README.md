# KoliseuOT Launcher

## Requisitos

- Node.js
- Rust
- Tauri CLI (`npm install`)

## Desenvolvimento

```bash
npm run tauri dev
```

## Gerar nova versão do Launcher

> Runbook completo (build assinado, publicação do update, rotação de chaves):
> [`docs/RELEASE.md`](docs/RELEASE.md).

### 1. Alterar a versão

Edite o arquivo `src-tauri/tauri.conf.json` e incremente o campo `version`:

```json
"version": "0.2.0",
```

### 2. Buildar

```bash
npm run tauri:build
```

Isso já configura as variáveis de assinatura automaticamente.

### 3. Arquivos gerados

Após o build, os arquivos estarão em:

```
src-tauri/target/release/bundle/nsis/
  KoliseuOT-Launcher_<versao>_x64-setup.exe    <- instalador / update
  KoliseuOT-Launcher_<versao>_x64-setup.exe.sig <- assinatura
```

### 4. Upload

Suba o `.exe` para o servidor (ex: `https://koliseuot.com.br/downloads/launcher/`).

### 5. Atualizar o endpoint

Atualize o endpoint `GET /api/launcher/updates/{{current_version}}` no site com:

- Nova `version`
- Nova `url` apontando para o `.exe` enviado
- Nova `signature` (conteúdo do arquivo `.sig`)

Exemplo de resposta do endpoint:

```json
{
  "version": "0.2.0",
  "notes": "Descrição das mudanças",
  "pub_date": "2026-03-22T00:00:00Z",
  "platforms": {
    "windows-x86_64": {
      "url": "https://koliseuot.com.br/downloads/launcher/KoliseuOT-Launcher_0.2.0_x64-setup.exe",
      "signature": "conteudo-do-arquivo-.sig"
    }
  }
}
```

Quando não houver update disponível, o endpoint deve retornar **status 204** (sem body).

### 6. Pronto

Os launchers abertos detectam o update automaticamente (ao abrir e a cada 15 minutos).

## Chaves de assinatura

- Chave privada: `src-tauri/keys/koliseuot.key` (NUNCA commitar)
- Chave pública: `src-tauri/keys/koliseuot.key.pub`
- Se perder a chave privada, não será possível assinar novos updates
