# Garante que o modelo e o binario whisper.cpp existem antes do build
# Executado automaticamente pelo beforeBuildCommand no tauri.conf.json

$ErrorActionPreference = "Stop"
$root = Join-Path $PSScriptRoot ".."

# ── Modelo base.pt (ggml) ──

$modelDir = Join-Path $root "src-tauri"
$modelDir = Join-Path $modelDir "models"
$modelDir = Join-Path $modelDir "whisper"
$modelFile = Join-Path $modelDir "base.pt"

if (-not (Test-Path $modelFile)) {
    Write-Host "Baixando modelo whisper base..." -ForegroundColor Yellow
    New-Item -ItemType Directory -Force -Path $modelDir | Out-Null

    # Use the model from the same release tag as the binary for compatibility
    $modelUrl = "https://huggingface.co/ggerganov/whisper.cpp/resolve/v1.7.4/ggml-base.bin"
    Write-Host "  $modelUrl" -ForegroundColor Gray
    try {
        Invoke-WebRequest -Uri $modelUrl -OutFile $modelFile -UseBasicParsing
    } catch {
        # Fallback to main branch if tagged version fails
        Write-Host "  Fallback para main..." -ForegroundColor Yellow
        Invoke-WebRequest -Uri "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin" -OutFile $modelFile -UseBasicParsing
    }
    $sizeMB = [math]::Round((Get-Item $modelFile).Length / 1MB, 1)
    Write-Host "Modelo baixado: ${sizeMB} MB" -ForegroundColor Green
} else { Write-Host "Modelo base.pt ja existe." -ForegroundColor Green }

# ── Binario whisper.cpp ──

$binDir = Join-Path $root "src-tauri"
$binDir = Join-Path $binDir "binaries"
$binFile = Join-Path $binDir "whisper-cli.exe"

if (-not (Test-Path $binFile)) {
    Write-Host "Baixando whisper.cpp..." -ForegroundColor Yellow
    New-Item -ItemType Directory -Force -Path $binDir | Out-Null
    $whisperVersion = "v1.7.4"
    $zipUrl = "https://github.com/ggerganov/whisper.cpp/releases/download/$whisperVersion/whisper-bin-x64.zip"
    $zipFile = Join-Path $env:TEMP "whisper-cpp-build.zip"
    Invoke-WebRequest -Uri $zipUrl -OutFile $zipFile -UseBasicParsing
    Expand-Archive -Path $zipFile -DestinationPath $binDir -Force
    Remove-Item $zipFile -Force
    $found = Get-ChildItem -Path $binDir -Recurse -Filter "whisper-cli.exe" | Select-Object -First 1
    if ($found) {
        if ($found.DirectoryName -ne $binDir) { Move-Item -Path $found.FullName -Destination $binFile -Force }
        # Also move any DLLs from the Release folder
        Get-ChildItem -Path $found.DirectoryName -Filter "*.dll" | Move-Item -Destination $binDir -Force
        # Clean up extracted folders, keep only exe and dlls
        Get-ChildItem -Path $binDir -Directory | Remove-Item -Recurse -Force
        Get-ChildItem -Path $binDir -File | Where-Object { $_.Extension -notin ".exe", ".dll" } | Remove-Item -Force
        Write-Host "whisper.cpp instalado." -ForegroundColor Green
    } else { Write-Host "AVISO: whisper-cli.exe nao encontrado no zip." -ForegroundColor Red }
} else { Write-Host "whisper.cpp ja existe." -ForegroundColor Green }

# ── ffmpeg (conversao de formatos nao-nativos do whisper.cpp) ──

$resDir = Join-Path $root "src-tauri"
$resDir = Join-Path $resDir "resources"
$ffFile = Join-Path $resDir "ffmpeg.exe"

if (-not (Test-Path $ffFile)) {
    Write-Host "Baixando ffmpeg..." -ForegroundColor Yellow
    New-Item -ItemType Directory -Force -Path $resDir | Out-Null
    # Build essential do gyan.dev (zip contem bin\ffmpeg.exe estatico)
    $ffUrl = "https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip"
    $ffZip = Join-Path $env:TEMP "ffmpeg-build.zip"
    $ffTmp = Join-Path $env:TEMP "ffmpeg-extract"
    try {
        Invoke-WebRequest -Uri $ffUrl -OutFile $ffZip -UseBasicParsing
        if (Test-Path $ffTmp) { Remove-Item $ffTmp -Recurse -Force }
        Expand-Archive -Path $ffZip -DestinationPath $ffTmp -Force
        $found = Get-ChildItem -Path $ffTmp -Recurse -Filter "ffmpeg.exe" | Select-Object -First 1
        if ($found) {
            Move-Item -Path $found.FullName -Destination $ffFile -Force
            $sizeMB = [math]::Round((Get-Item $ffFile).Length / 1MB, 1)
            Write-Host "ffmpeg instalado: ${sizeMB} MB" -ForegroundColor Green
        } else {
            Write-Host "AVISO: ffmpeg.exe nao encontrado no zip." -ForegroundColor Red
        }
        Remove-Item $ffZip -Force
        Remove-Item $ffTmp -Recurse -Force
    } catch {
        Write-Host "AVISO: falha ao baixar ffmpeg ($_). Formatos como opus/mpga/video podem nao converter." -ForegroundColor Red
    }
} else { Write-Host "ffmpeg ja existe." -ForegroundColor Green }

Write-Host "Pronto para build!" -ForegroundColor Cyan
