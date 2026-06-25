# Transcrever

Aplicativo desktop (Windows) para **transcrição de áudio/vídeo em lote**, construído com
[Tauri 2](https://v2.tauri.app/) (Rust) + React + TypeScript. Suporta dois motores de
transcrição Whisper e organiza o trabalho por **projetos**, com fila, revisão e exportação.

## Funcionalidades

- **Escaneamento recursivo** de pastas (áudio e vídeo: mp3, wav, m4a, opus, mp4, mkv, ...).
- **Fila** com processamento paralelo configurável, parar/retomar e reprocessamento de erros.
- **Projetos**: agrupe pastas, defina um perfil de transcrição por projeto, arquive/exclua.
- **Revisão**: busca em todas as transcrições, filtros (formato, tamanho, data), top palavras,
  edição inline, player de áudio, marcador de confiança e exportação `.txt`.
- **Dois backends**:
  - **whisper.cpp** — binário embarcado, **não precisa de Python**. Modelo = arquivo `ggml/gguf` (`.bin`).
  - **faster-whisper** — mais rápido (especialmente em GPU NVIDIA), **exige Python**. Modelo = pasta CTranslate2.

## Pré-requisitos

### Para usar (apenas rodar o `.exe`)
- **Windows x64**.
- Para **faster-whisper**: **Python 3.10+** instalado. As dependências podem ser instaladas
  pela própria tela **Configurações → Dependências** (botões de instalar), ou manualmente:
  ```
  pip install faster-whisper
  # suporte GPU (NVIDIA, opcional):
  pip install nvidia-cublas-cu12 nvidia-cudnn-cu12
  ```
- Para **whisper.cpp**: nada além do app (binário e modelo padrão vêm embarcados).

### Para desenvolver / gerar o `.exe`
- **Node.js 18+** e npm.
- **Rust** (stable) + toolchain MSVC — instale via [rustup](https://rustup.rs/).
- **Dependências do Tauri no Windows**: WebView2 (já vem no Windows 10/11) e as
  *Build Tools* do Visual Studio (C++). Veja: https://v2.tauri.app/start/prerequisites/
- **PowerShell** (os scripts de setup usam PowerShell).

## Instalação do projeto

```bash
git clone <repo>
cd transcrever
npm install
```

## Rodar em desenvolvimento

```bash
npm run tauri dev
```
Isso sobe o Vite (frontend) e compila/roda o app Tauri com hot-reload.

> O modelo padrão (`ggml-base`) e o binário `whisper-cli.exe` são baixados automaticamente
> na primeira build pelo script `scripts/ensure-model.ps1`. Para baixar manualmente um modelo
> diferente do whisper.cpp:
> ```
> npm run setup            # baixa o modelo base
> # ou um modelo especifico:
> powershell -ExecutionPolicy Bypass -File scripts/setup-model.ps1 -Model large-v3
> ```

## Gerar o executável / instalador

```bash
npm run tauri build
```
O `beforeBuildCommand` garante o modelo + binário e compila o frontend. Os artefatos saem em:
```
src-tauri/target/release/                       # transcrever.exe
src-tauri/target/release/bundle/                # instaladores (msi / nsis)
```

### Conversão de formatos (ffmpeg)
O whisper.cpp só decodifica `wav/mp3/flac/ogg` com segurança; os demais formatos (opus, mpga,
m4a, vídeos...) são convertidos para WAV via **ffmpeg**. Coloque o `ffmpeg.exe` em
`src-tauri/resources/ffmpeg.exe` para que seja empacotado; na ausência dele, o app tenta o
`ffmpeg` do PATH do sistema.

## Estrutura

```
src/                      # frontend React (App.tsx, styles.css, types.ts)
src-tauri/
  src/
    commands.rs           # comandos expostos ao frontend (Tauri)
    db.rs                 # acesso ao SQLite (sqlx) e migrações
    queue.rs              # enfileiramento de mídia
    scanner.rs            # varredura recursiva de pastas
    export.rs             # exportação .txt
    backend/
      whisper_cpp.rs      # adaptador whisper.cpp (binário)
      faster_whisper.rs   # adaptador faster-whisper (Python)
  migrations/             # migrações SQL (sqlx)
  scripts/
    faster_whisper_transcribe.py  # script chamado pelo backend faster-whisper
scripts/
  ensure-model.ps1        # baixa modelo + binário (usado no build)
  setup-model.ps1         # baixa um modelo whisper.cpp especifico
  repair_db.py            # reparo do banco (remove jobs duplicados)
```

## Backends e dicas de qualidade/velocidade

- A **qualidade** depende sobretudo do **tamanho do modelo**: `base < small < medium < large-v3`.
- **GPU NVIDIA moderna (≥ Volta)**: faster-whisper + `cuda` + `float16` é o mais rápido.
- **GPU antiga (Maxwell/Pascal, ex.: Quadro M4000)**: usa `cuda` + **`float32`** (não há float16).
- **Sem GPU / simplicidade**: whisper.cpp com um modelo `ggml` (ex.: `medium`/`large-v3`).
- **Concorrência** (Configurações): em CPU, `threads × simultâneos ≈ núcleos`. Em GPU, use **1**.

## Banco de dados

SQLite em `%APPDATA%\br.local.transcrever\transcrever.sqlite`. As migrações rodam
automaticamente na inicialização.

### Reparar banco (remover duplicados)

Versões antigas podiam duplicar jobs ao re-escanear. O app já limpa duplicados na
inicialização, mas há também um script manual (faz backup `.bak` antes):

```bash
python scripts/repair_db.py
# ou apontando o caminho:
python scripts/repair_db.py "C:\Users\<voce>\AppData\Roaming\br.local.transcrever\transcrever.sqlite"
```
Mantém, por arquivo, apenas um job (concluído > erro > processando > pendente) e remove o resto.
**Os arquivos de áudio originais nunca são tocados.**
