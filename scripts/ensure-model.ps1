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
    $url = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin"
    Invoke-WebRequest -Uri $url -OutFile $modelFile -UseBasicParsing
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
    $zipUrl = "https://github.com/ggerganov/whisper.cpp/releases/latest/download/whisper-bin-x64.zip"
    $zipFile = Join-Path $env:TEMP "whisper-cpp-build.zip"
    Invoke-WebRequest -Uri $zipUrl -OutFile $zipFile -UseBasicParsing
    Expand-Archive -Path $zipFile -DestinationPath $binDir -Force
    Remove-Item $zipFile -Force
    $found = Get-ChildItem -Path $binDir -Recurse -Filter "whisper-cli.exe" | Select-Object -First 1
    if ($found) {
        if ($found.DirectoryName -ne $binDir) { Move-Item -Path $found.FullName -Destination $binFile -Force }
        Get-ChildItem -Path $binDir -Directory | Remove-Item -Recurse -Force
        Get-ChildItem -Path $binDir -File | Where-Object { $_.Name -ne "whisper-cli.exe" } | Remove-Item -Force
        Write-Host "whisper.cpp instalado." -ForegroundColor Green
    } else { Write-Host "AVISO: whisper-cli.exe nao encontrado no zip." -ForegroundColor Red }
} else { Write-Host "whisper.cpp ja existe." -ForegroundColor Green }

Write-Host "Pronto para build!" -ForegroundColor Cyan
