<div align="center">
  
# 🌅 Horizon

**Your Premium, Uncensored Personal AI Assistant.**
<br>
*Built with Rust, Tauri v2, Vanilla JS, Ollama, and ComfyUI.*

[![Version](https://img.shields.io/badge/version-v2.1.0-d4af37.svg)](https://github.com/suarezjoris/Horizon/releases)
[![Platform](https://img.shields.io/badge/platform-Linux%20%7C%20Windows-00f2ff.svg)]()
[![Rust](https://img.shields.io/badge/rust-backend-orange.svg)]()

</div>

---

## 🔮 Overview

Horizon is a private, locally-hosted AI ecosystem designed to be the ultimate digital companion. Moving away from cloud-dependent services, Horizon runs entirely on your hardware, ensuring absolute privacy and uncensored interactions. 

Wrapped in a custom **Hextech / Arcane** aesthetic (Dark Glassmorphism, Cinzel typography, and Clockwork motion), Horizon integrates advanced text generation, image creation, video generation, roleplaying, and knowledge management into a single, seamless native desktop application.

## ✨ Core Features

- **💬 Intelligent LLM Chat**: Powered by `qwen2.5-coder:14b` (via Ollama). Horizon can reason, code, and chat fluidly.
- **🌐 Autonomous Web Search**: If Horizon doesn't know a fact, it automatically triggers a Python-based DuckDuckGo search to verify information before answering.
- **🖼️ Image Generation**: Deep integration with **ComfyUI**. Generate stunning images using SDXL/Pony models directly from the chat or the dedicated Image tab.
- **🎬 Cinema (Video Generation)**: Text-to-video and image-to-video via **ComfyUI + Wan 2.2** (WanVideoWrapper). Renders animated clips with a "Past Renders" gallery and plays them in your system player. *Linux + CUDA GPU; block-swap lets the 14B model run on 12 GB VRAM (image-to-video gives the best results).*
- **🎭 Character Roleplay**: Import TavernAI PNG character cards and roleplay with local LLMs.
- **📝 Obsidian-Style Notes**: A built-in markdown editor and Vault system to manage your personal knowledge base.
- **🎤 Audio Import**: Speak directly to Horizon using the integrated microphone interface, powered by local **Faster-Whisper** transcription.

## 🛠️ System Architecture

Horizon is built for performance and absolute local control:
- **Frontend**: Ultra-light Vanilla JS and CSS. No heavy frameworks.
- **Backend**: Rust (Tauri v2) for native OS integration and blazing-fast IPC.
- **AI Engine**: Ollama (LLM) and ComfyUI (Vision/Diffusion).
- **Security**: Strict CSP, validated system paths, and isolated UI panels.

## 🚀 Installation

### 🐧 Linux (Arch)

1. **Clone the repository:**
   ```bash
   git clone https://github.com/suarezjoris/Horizon.git
   cd Horizon
   ```

2. **Run the Master Installer:**
   ```bash
   ./install.sh
   ```
   *This script will automatically install system dependencies, Rust, Tauri CLI, download the required Ollama models, and set up the ComfyUI virtual environment.*

3. **Launch:**
   Open "Horizon" from your Linux application launcher, or run `~/.local/bin/horizon`.

### 🪟 Windows

1. **Download the installer:** grab the latest `Horizon_x.y.z_x64_en-US.msi` (or `..._x64-setup.exe`) from the [Releases page](https://github.com/suarezjoris/Horizon/releases).

   > The installer is unsigned, so Windows SmartScreen may warn *"Windows protected your PC"*. Click **More info → Run anyway**.

2. **Install it**, then provision the runtime (Python env, ComfyUI, models):
   ```powershell
   powershell -ExecutionPolicy Bypass -File setup-windows.ps1
   ```
   *Prerequisites: [Ollama for Windows](https://ollama.com) and [Git](https://git-scm.com/download/win). The script clones the source to `%USERPROFILE%\Projects\Horizon` and sets up the Python venv, ComfyUI, and the Pony XL model.*

3. **Launch** "Horizon" from the Start Menu (make sure Ollama is running).

   > **Note:** Chat, Roleplay, Notes, Web Search, Audio and Image Generation work on Windows. The **Code tab (Aider)** and **Cinema (video generation)** are Linux-only for now.

## 🛡️ Diagnostics & Auto-Repair

Horizon v2.05 features a built-in **System Health Diagnostic**. On launch, it verifies:
- Ollama server status & model availability.
- Python (`uv`) environment.
- ComfyUI paths & virtual environments.
- Vault directory integrity.

If anything breaks (e.g., you moved a folder), the UI will provide a **"Fix"** button to auto-repair the configuration.

You can also run a deep maintenance check at any time from the terminal:
```bash
./update.sh
```
*This will pull the latest code, verify models, rebuild the Rust binary, and fix broken paths.*

---

<div align="center">
  <i>"Cultivating dreams into digital reality."</i>
</div>
