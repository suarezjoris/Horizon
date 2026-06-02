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

# 5. Install UV (Python package manager)
echo "📦 Installing uv..."
if ! command -v uv &> /dev/null; then
    curl -LsSf https://astral.sh/uv/install.sh | sh
    source "$HOME/.local/bin/env" || true
fi

# 6. ComfyUI Setup
echo "🖼 Setting up ComfyUI..."
if [ ! -d "$PROJECT_ROOT/ComfyUI" ]; then
    git clone https://github.com/comfyanonymous/ComfyUI.git "$PROJECT_ROOT/ComfyUI"
fi

cd "$PROJECT_ROOT/ComfyUI"
if [ ! -d "venv" ]; then
    echo "Creating Python 3.12 virtual environment for ComfyUI to ensure PyTorch compatibility..."
    ~/.local/bin/uv venv --python 3.12 venv
    ~/.local/bin/uv pip install torch torchvision torchaudio --index-url https://download.pytorch.org/whl/cu121
    ~/.local/bin/uv pip install -r requirements.txt
fi

# Download Pony XL if missing
mkdir -p "$MODELS_DIR"
if [ ! -f "$MODELS_DIR/ponyDiffusionV6XL_v6.safetensors" ]; then
    echo "📥 Downloading Pony Diffusion V6 XL (6.5GB)..."
    wget -O "$MODELS_DIR/ponyDiffusionV6XL_v6.safetensors" \
        https://huggingface.co/LyliaEngine/Pony_Diffusion_V6_XL/resolve/main/ponyDiffusionV6XL_v6StartWithThis.safetensors
fi
cd "$PROJECT_ROOT"

# 7. Aider Setup (via UV)
echo "💻 Setting up Aider (Code Agent)..."
# Install Aider with Python 3.12 to avoid audioop issues on 3.14
~/.local/bin/uv tool install --python 3.12 'aider-chat[playwright]' --force

# 8. Vault Initialization
echo "📂 Initializing Vault..."
mkdir -p "$VAULT_PATH/memory" "$VAULT_PATH/images" "$VAULT_PATH/characters"

# 9. Build Application
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

# 10. System Integration (Desktop Entry & Icon)
echo "🖥️ Integrating with system menu..."
BIN_DEST="$HOME/.local/bin/horizon"
ICON_DEST="$HOME/.local/share/icons/horizon.png"
DESKTOP_DEST="$HOME/.local/share/applications/horizon.desktop"

# Copy binary
cp "$PROJECT_ROOT/src-tauri/target/release/horizon" "$BIN_DEST"
chmod +x "$BIN_DEST"

# Copy icon (using the one from tauri)
mkdir -p "$HOME/.local/share/icons"
cp "$PROJECT_ROOT/src-tauri/icons/icon.png" "$ICON_DEST"

# Create .desktop file
cat <<EOF > "$DESKTOP_DEST"
[Desktop Entry]
Name=Horizon
Comment=Personal AI Assistant (LLM, Image, Roleplay, Code)
Exec=$BIN_DEST
Icon=$ICON_DEST
Terminal=false
Type=Application
Categories=Development;Utility;
Keywords=ai;llm;chat;code;
EOF

update-desktop-database "$HOME/.local/share/applications" || true

echo ""
echo "✅ Installation Complete!"
echo "Horizon is now available in your application menu."
echo "You can launch it by searching for 'Horizon' or by running '$BIN_DEST' directly."
echo ""
echo "Note: If it doesn't appear immediately, you might need to restart your launcher or run 'update-desktop-database ~/.local/share/applications'"
