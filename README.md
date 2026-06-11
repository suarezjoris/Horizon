<div align="center">
  
# 🌅 Horizon V4

**The Ultimate Agentic, Uncensored Personal AI Ecosystem.**
<br>
*Native Rust (Tauri v2), Vanilla JS, Ollama, and ComfyUI.*

[![Version](https://img.shields.io/badge/version-v4.0.0-d4af37.svg)](https://github.com/suarezjoris/Horizon/releases)
[![Platform](https://img.shields.io/badge/platform-Linux-00f2ff.svg)]()
[![Rust](https://img.shields.io/badge/rust-backend-orange.svg)]()

</div>

---

## 🔮 Overview

Horizon V4 is a major evolution from a "Chat Assistant" to a fully **Agentic Ecosystem**. It is a private, locally-hosted digital companion that doesn't just talk—it **acts**. Built with a focus on absolute privacy, uncensored intelligence, and high-performance local RAG.

Wrapped in its signature **Hextech / Arcane** aesthetic, V4 introduces a native tool-calling loop, autonomous background daemons, and a tactical command center.

## ✨ What's New in V4

### 🤖 Native Agentic Loop (Ollama Tool Calling)
- **Tool-Calling Engine**: Replaced legacy tag+regex parsing with **native tool calling** (optimized for `qwen2.5-coder:14b`).
- **Autonomous Action**: Horizon can now independently **read/write/edit files, run terminal commands, and search the web** in a multi-turn reasoning loop.
- **Enhanced Reliability**: New "llm-done" payload system ensures final responses are never lost, even after complex tool sequences.

### 🛡️ ARMATA Tactical Dashboard
- **Live Command Center**: A new 2×2 agent grid for monitoring background processes.
- **Agent Daemons**:
  - **Archivist**: Real-time file watcher that automatically indexes your workspace.
  - **Vanguard**: Background RSS/Atom scanner with LLM summarization.
  - **Forge**: Autonomous vault health automation and Hub discovery.
- **VRAM Monitor**: Real-time VRAM tracking and resource queueing to prevent OOMs when running LLMs and Diffusion models simultaneously.

### 🛰️ Antenna (Mobile Bridge)
- **Remote Access**: Secure HTTP bridge (Axum-based) allowing you to send commands to your local Horizon instance from mobile devices (Bearer token authenticated).

### 🛠️ Infrastructure & Security
- **Cross-Distro Support**: Refactored `install.sh` and `update.sh` with native support for Arch, Debian/Ubuntu, Fedora, and openSUSE.
- **Security Sandboxing**: New `bwrap` (Bubblewrap) integration for safer terminal command execution.
- **Command Allowlist**: Hardcoded security guards prevent RCE via the mobile bridge.

## 🚀 Installation

### 🐧 Linux (Recommended)

Horizon V4 is optimized for Linux with CUDA support.

1. **Clone the repository:**
   ```bash
   git clone https://github.com/suarezjoris/Horizon.git
   cd Horizon
   ```

2. **Run the Master Installer:**
   ```bash
   chmod +x install.sh
   ./install.sh
   ```
   *This script automatically detects your package manager (pacman/apt/dnf/zypper), installs system dependencies, Rust, Ollama, and sets up the ComfyUI/Python environment.*

3. **Launch:**
   Open "Horizon" from your application launcher, or run:
   ```bash
   ./update.sh --run
   ```

### 🪟 Windows (Partial Support)

1. **Prerequisites**: Install [Ollama for Windows](https://ollama.com) and [Git](https://git-scm.com/download/win).
2. **Setup**:
   ```powershell
   powershell -ExecutionPolicy Bypass -File setup-windows.ps1
   ```
3. **Launch**: Open the Horizon executable (ensure Ollama is running).
   *Note: Background daemons (ARMATA) and certain agent tools are currently Linux-optimized.*

## 🛠️ Maintenance & Updates

Keep your V4 instance healthy with the unified update utility:
```bash
./update.sh
```
This script performs a deep health check, pulls code updates, migrates settings, and re-compiles the Rust core if necessary.

---

<div align="center">
  <i>"Local intelligence. Absolute freedom."</i>
</div>
