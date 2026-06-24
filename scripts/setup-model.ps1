# Download do modelo whisper.cpp para o Transcrever
# Execute: .\scripts\setup-model.ps1

param(
    [string]$Model = "base",
    [string]$OutDir = "src-tauri\models\whisper"
)

$ErrorActionPreference = "Stop"

$modelMap = @{
    "tiny"   = "ggml-tiny.bin"
    "base"   = "ggml-base.bin"
    "small"  = "ggml-small.bin"
    "medium" = "ggml-medium.bin"
    "large-v3" = "ggml-large-v3.bin"
    "large-v3-turbo" = "ggml-large-v3-turbo.bin"
}

$fileName = $modelMap[$Model]
if (-not $fileName) {
    Write-Error "Modelo '$Model' desconhecido. Use: tiny, base, small, medium, large-v3, large-v3-turbo"
    exit 1
}

$url = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/$fileName"
$outDir = Join-Path $PSScriptRoot ".." $OutDir
New-Item -ItemType Directory -Force -Path $outDir | Out-Null

$outFile = Join-Path $outDir "base.pt"
Write-Host "Baixando $fileName ($Model)..." -ForegroundColor Cyan
Write-Host "  De: $url" -ForegroundColor Gray
Write-Host "  Para: $outFile" -ForegroundColor Gray

Invoke-WebRequest -Uri $url -OutFile $outFile -UseBasicParsing

$size = [math]::Round((Get-Item $outFile).Length / 1MB, 1)
Write-Host "Concluido! ${size} MB baixados." -ForegroundColor Green
