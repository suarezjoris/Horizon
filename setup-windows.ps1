# Horizon v2 - Windows Runtime Setup
# Run after installing the .msi:  powershell -ExecutionPolicy Bypass -File setup-windows.ps1
#
# The .msi ships only the Horizon binary. This script provisions the runtime that the
# Python-backed features need (web search, audio transcription, ComfyUI image generation).
# It mirrors the Linux install.sh, but does NOT build Rust (the .msi is the binary).
#
# NOTE: The Code tab (Aider) is not supported on Windows yet and is skipped here.

$ErrorActionPreference = "Stop"

$ProjectRoot = Join-Path $env:USERPROFILE "Projects\Horizon"
$Vault       = Join-Path $env:USERPROFILE "Documents\Claude RAG"
$ComfyUI     = Join-Path $ProjectRoot "ComfyUI"
$ModelsDir   = Join-Path $ComfyUI "models\checkpoints"
$AppVenv     = Join-Path $ProjectRoot ".venv"
$ConfigDir   = Join-Path $env:APPDATA "horizon"
$DataDir     = Join-Path $env:LOCALAPPDATA "horizon"
$RepoUrl     = "https://github.com/suarezjoris/Horizon.git"

Write-Host "Starting Horizon v2 Windows setup..." -ForegroundColor Cyan
Write-Host "Project root: $ProjectRoot"

# 0. Prerequisite: git
if (-not (Get-Command git -ErrorAction SilentlyContinue)) {
    Write-Error "git not found. Install Git for Windows (https://git-scm.com/download/win) and re-run."
}

# 1. Source checkout at the expected location (Horizon resolves scripts/.venv from ~/Projects/Horizon)
if (-not (Test-Path $ProjectRoot)) {
    Write-Host "Cloning Horizon source to $ProjectRoot ..." -ForegroundColor Yellow
    git clone $RepoUrl $ProjectRoot
}

# 2. uv (Python package manager)
$Uv = Join-Path $env:USERPROFILE ".local\bin\uv.exe"
if (-not (Test-Path $Uv)) {
    if (Get-Command uv -ErrorAction SilentlyContinue) {
        $Uv = "uv"
    } else {
        Write-Host "Installing uv..." -ForegroundColor Yellow
        Invoke-RestMethod https://astral.sh/uv/install.ps1 | Invoke-Expression
    }
}

# 3. App venv for web search + transcription
Write-Host "Creating Python venv at $AppVenv ..." -ForegroundColor Yellow
$AppVenvPython = Join-Path $AppVenv "Scripts\python.exe"
if (-not (Test-Path $AppVenvPython)) {
    & $Uv venv --python 3.12 $AppVenv
}
& $Uv pip install --python $AppVenvPython ddgs faster-whisper

# 4. Ollama models
if (Get-Command ollama -ErrorAction SilentlyContinue) {
    Write-Host "Pulling Ollama models..." -ForegroundColor Yellow
    ollama pull dolphin-mixtral:8x7b
    ollama pull nomic-embed-text:latest
    ollama pull qwen2.5-coder:14b
    ollama cp qwen2.5-coder:14b gpt-4o 2>$null
} else {
    Write-Host "WARNING: Ollama not found. Install it from https://ollama.com and re-run model pulls." -ForegroundColor Red
}

# 5. ComfyUI (image generation)
Write-Host "Setting up ComfyUI..." -ForegroundColor Yellow
if (-not (Test-Path $ComfyUI)) {
    git clone https://github.com/comfyanonymous/ComfyUI.git $ComfyUI
}
$ComfyVenvPython = Join-Path $ComfyUI "venv\Scripts\python.exe"
if (-not (Test-Path $ComfyVenvPython)) {
    & $Uv venv --python 3.12 (Join-Path $ComfyUI "venv")
    & $Uv pip install --python $ComfyVenvPython torch torchvision torchaudio --index-url https://download.pytorch.org/whl/cu121
    & $Uv pip install --python $ComfyVenvPython -r (Join-Path $ComfyUI "requirements.txt")
}

# Download Pony XL checkpoint if missing
New-Item -ItemType Directory -Force -Path $ModelsDir | Out-Null
$Checkpoint = Join-Path $ModelsDir "ponyDiffusionV6XL_v6.safetensors"
if (-not (Test-Path $Checkpoint)) {
    Write-Host "Downloading Pony Diffusion V6 XL (6.5GB)..." -ForegroundColor Yellow
    curl.exe -L -o $Checkpoint "https://huggingface.co/LyliaEngine/Pony_Diffusion_V6_XL/resolve/main/ponyDiffusionV6XL_v6StartWithThis.safetensors"
}

# 6. Vault + app directories
Write-Host "Initializing Vault and config..." -ForegroundColor Yellow
New-Item -ItemType Directory -Force -Path (Join-Path $Vault "memory")     | Out-Null
New-Item -ItemType Directory -Force -Path (Join-Path $Vault "images")     | Out-Null
New-Item -ItemType Directory -Force -Path (Join-Path $Vault "characters") | Out-Null
New-Item -ItemType Directory -Force -Path $ConfigDir | Out-Null
New-Item -ItemType Directory -Force -Path $DataDir   | Out-Null

# 7. Settings file
$Settings = [ordered]@{
    vault_path      = $Vault
    llm_model       = "dolphin-mixtral:8x7b"
    roleplay_model  = "llama3.1:8b"
    comfyui_path    = (Join-Path $ComfyUI "main.py")
    embeddings_path = (Join-Path $DataDir "embeddings.bin")
    image_rating    = "rating_safe"
}
$Settings | ConvertTo-Json | Set-Content -Path (Join-Path $ConfigDir "settings.json") -Encoding UTF8

Write-Host ""
Write-Host "Setup complete!" -ForegroundColor Green
Write-Host "Launch Horizon from the Start Menu. Make sure Ollama is running."
