# Garante que o modelo base.pt existe antes do build do Tauri
# Executado automaticamente pelo beforeBuildCommand no tauri.conf.json

$ErrorActionPreference = "Stop"

$modelDir = Join-Path $PSScriptRoot ".." "src-tauri" "models" "whisper"
$modelFile = Join-Path $modelDir "base.pt"

if (Test-Path $modelFile) {
    Write-Host "Modelo base.pt ja existe: $modelFile" -ForegroundColor Green
    exit 0
}

Write-Host "Modelo base.pt nao encontrado. Baixando..." -ForegroundColor Yellow

New-Item -ItemType Directory -Force -Path $modelDir | Out-Null

$url = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin"
Write-Host "  De: $url" -ForegroundColor Gray
Write-Host "  Para: $modelFile" -ForegroundColor Gray

Invoke-WebRequest -Uri $url -OutFile $modelFile -UseBasicParsing

$sizeMB = [math]::Round((Get-Item $modelFile).Length / 1MB, 1)
Write-Host "Modelo baixado: ${sizeMB} MB" -ForegroundColor Green
