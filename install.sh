#!/usr/bin/env bash
set -e

# Horizon v2 — Master Installation Script (Arch Linux)
# This script automates the setup of LLM, Image Gen, and Roleplay modules.

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
VAULT_PATH="$HOME/Documents/Claude RAG"
MODELS_DIR="$PROJECT_ROOT/ComfyUI/models/checkpoints"

echo "🚀 Starting Horizon v2 Installation..."

# 1. System Dependencies (Arch Linux)
echo "📦 Installing system dependencies..."
sudo pacman -S --needed --noconfirm \
    webkit2gtk-4.1 base-devel curl wget git ctags ripgrep \
    openblas gcc-fortran nspr nss at-spi2-core libdrm mesa \
    libxcomposite libxdamage libxfixes libxrandr alsa-lib pango cairo

# 2. Rust Toolchain
if ! command -v cargo &> /dev/null; then
    echo "🦀 Installing Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
else
    echo "✅ Rust already installed."
fi

# 3. Tauri CLI
if ! cargo tauri --version &> /dev/null; then
    echo "🛠 Installing Tauri CLI..."
    cargo install tauri-cli --version "^2.0.0"
fi

# 4. Ollama & Models
echo "🧠 Setting up Ollama models..."
if ! command -v ollama &> /dev/null; then
    echo "⚠️ Ollama not found. Please install it from https://ollama.com"
else
    ollama pull dolphin-mixtral:8x7b
    ollama pull nomic-embed-text:latest
    ollama pull qwen2.5-coder:14b
    # Create alias for tool-calling compatibility
    ollama cp qwen2.5-coder:14b gpt-4o || true
fi

# 5. ComfyUI Setup
echo "🖼 Setting up ComfyUI..."
if [ ! -d "$PROJECT_ROOT/ComfyUI" ]; then
    git clone https://github.com/comfyanonymous/ComfyUI.git "$PROJECT_ROOT/ComfyUI"
fi

cd "$PROJECT_ROOT/ComfyUI"
if [ ! -d "venv" ]; then
    python -m venv venv
    ./venv/bin/pip install torch torchvision torchaudio --index-url https://download.pytorch.org/whl/cu121
    ./venv/bin/pip install -r requirements.txt
fi

# Download Pony XL if missing
mkdir -p "$MODELS_DIR"
if [ ! -f "$MODELS_DIR/ponyDiffusionV6XL_v6.safetensors" ]; then
    echo "📥 Downloading Pony Diffusion V6 XL (6.5GB)..."
    wget -O "$MODELS_DIR/ponyDiffusionV6XL_v6.safetensors" \
        https://huggingface.co/LyliaEngine/Pony_Diffusion_V6_XL/resolve/main/ponyDiffusionV6XL_v6StartWithThis.safetensors
fi
cd "$PROJECT_ROOT"

# 6. Aider Setup (via UV)
echo "💻 Setting up Aider (Code Agent)..."
if ! command -v uv &> /dev/null; then
    curl -LsSf https://astral.sh/uv/install.sh | sh
    source "$HOME/.local/bin/env" || true
fi
# Install Aider with Python 3.12 to avoid audioop issues on 3.14
~/.local/bin/uv tool install --python 3.12 'aider-chat[playwright]' --force

# 7. Vault Initialization
echo "📂 Initializing Vault..."
mkdir -p "$VAULT_PATH/memory" "$VAULT_PATH/images" "$VAULT_PATH/characters"

# 8. Build Application
echo "🏗 Building Horizon v2 Release..."
cd "$PROJECT_ROOT/src-tauri"
# Update tauri settings to point to the correct ComfyUI path
mkdir -p "$HOME/.config/horizon"
cat <<EOF > "$HOME/.config/horizon/settings.json"
{
  "vault_path": "$VAULT_PATH",
  "llm_model": "dolphin-mixtral:8x7b",
  "roleplay_model": "llama3.1:8b",
  "comfyui_path": "$PROJECT_ROOT/ComfyUI/main.py",
  "embeddings_path": "$HOME/.local/share/horizon/embeddings.bin"
}
EOF

source "$HOME/.cargo/env" || true
cargo tauri build

echo ""
echo "✅ Installation Complete!"
echo "Your standalone application is ready in:"
echo "$PROJECT_ROOT/src-tauri/target/release/bundle/appimage/"
echo ""
echo "You can now run Horizon from your application menu or by double-clicking the AppImage."
