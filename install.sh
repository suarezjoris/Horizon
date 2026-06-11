#!/usr/bin/env bash
set -e

# Horizon v2 — Installation Script (Linux multi-distro)
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
VAULT_PATH="$HOME/Documents/Claude RAG"
MODELS_DIR="$PROJECT_ROOT/ComfyUI/models/checkpoints"
WORKSPACE_DIR="$PROJECT_ROOT/workspace"

echo "🚀 Starting Horizon Installation..."

# ── 1. Detect distro ──────────────────────────────────────────────────────────
echo "📦 Installing system dependencies..."
if command -v pacman &>/dev/null; then
    sudo pacman -S --needed --noconfirm \
        webkit2gtk-4.1 base-devel curl wget git bubblewrap \
        openblas nspr nss at-spi2-core libdrm mesa \
        libxcomposite libxdamage libxfixes libxrandr alsa-lib pango cairo \
        python python-pip
elif command -v apt-get &>/dev/null; then
    sudo apt-get update -qq
    sudo apt-get install -y \
        libwebkit2gtk-4.1-dev build-essential curl wget git bubblewrap \
        libssl-dev libgtk-3-dev libsoup-3.0-dev \
        libjavascriptcoregtk-4.1-dev librsvg2-dev \
        python3 python3-pip python3-venv
elif command -v dnf &>/dev/null; then
    sudo dnf install -y \
        webkit2gtk4.1-devel gcc make curl wget git bubblewrap \
        openssl-devel gtk3-devel libsoup3-devel \
        python3 python3-pip
elif command -v zypper &>/dev/null; then
    sudo zypper install -y \
        webkit2gtk3-devel gcc make curl wget git bubblewrap \
        libopenssl-devel gtk3-devel libsoup3-devel \
        python3 python3-pip
else
    echo "⚠️  Unknown distro — install manually: libwebkit2gtk-4.1, build tools, bubblewrap, python3"
fi

# ── 2. Rust ───────────────────────────────────────────────────────────────────
if ! command -v cargo &>/dev/null; then
    echo "🦀 Installing Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
else
    echo "✅ Rust already installed."
fi
source "$HOME/.cargo/env" || true

# ── 3. Tauri CLI ──────────────────────────────────────────────────────────────
if ! cargo tauri --version &>/dev/null; then
    echo "🛠  Installing Tauri CLI..."
    cargo install tauri-cli --version "^2.0.0"
fi

# ── 4. uv (Python package manager) ───────────────────────────────────────────
if ! command -v uv &>/dev/null; then
    echo "📦 Installing uv..."
    curl -LsSf https://astral.sh/uv/install.sh | sh
    source "$HOME/.local/bin/env" || true
    export PATH="$HOME/.local/bin:$PATH"
fi

# ── 5. Python venv for search_web.py ─────────────────────────────────────────
echo "🐍 Setting up search venv..."
if [ ! -d "$PROJECT_ROOT/.venv" ]; then
    ~/.local/bin/uv venv --python 3.12 "$PROJECT_ROOT/.venv" 2>/dev/null \
        || python3 -m venv "$PROJECT_ROOT/.venv"
fi
"$PROJECT_ROOT/.venv/bin/pip" install -q --upgrade \
    ddgs readability-lxml requests

# ── 6. Ollama & Models ────────────────────────────────────────────────────────
echo "🧠 Setting up Ollama..."
if ! command -v ollama &>/dev/null; then
    echo "📥 Installing Ollama..."
    curl -fsSL https://ollama.com/install.sh | sh
fi

echo "📥 Pulling models (this may take a while)..."
ollama pull qwen2.5:14b
ollama pull llama3.1:8b
ollama pull nomic-embed-text:latest
ollama pull moondream:latest

# ── 7. ComfyUI Setup ──────────────────────────────────────────────────────────
echo "🖼  Setting up ComfyUI..."
if [ ! -d "$PROJECT_ROOT/ComfyUI" ]; then
    git clone https://github.com/comfyanonymous/ComfyUI.git "$PROJECT_ROOT/ComfyUI"
fi

# Verify clone completed (requirements.txt must exist)
if [ ! -f "$PROJECT_ROOT/ComfyUI/requirements.txt" ]; then
    echo "❌ ComfyUI clone seems incomplete (requirements.txt missing). Re-cloning..."
    rm -rf "$PROJECT_ROOT/ComfyUI"
    git clone https://github.com/comfyanonymous/ComfyUI.git "$PROJECT_ROOT/ComfyUI"
fi

cd "$PROJECT_ROOT/ComfyUI"
if [ ! -d "venv" ]; then
    echo "🐍 Creating ComfyUI venv (Python 3.12)..."
    ~/.local/bin/uv venv --python 3.12 venv 2>/dev/null \
        || python3 -m venv venv
    ~/.local/bin/uv pip install \
        torch torchvision torchaudio \
        --index-url https://download.pytorch.org/whl/cu121 \
        --python venv/bin/python3 2>/dev/null \
        || venv/bin/python3 -m pip install torch torchvision torchaudio \
            --index-url https://download.pytorch.org/whl/cu121
    venv/bin/python3 -m pip install -r requirements.txt -q
    venv/bin/python3 -m pip install opencv-python-headless imageio-ffmpeg -q
fi

mkdir -p "$MODELS_DIR"
if [ ! -f "$MODELS_DIR/ponyDiffusionV6XL_v6.safetensors" ]; then
    echo "📥 Downloading Pony Diffusion V6 XL (6.5 GB)..."
    wget -q --show-progress \
        -O "$MODELS_DIR/ponyDiffusionV6XL_v6.safetensors" \
        "https://huggingface.co/LyliaEngine/Pony_Diffusion_V6_XL/resolve/main/ponyDiffusionV6XL_v6StartWithThis.safetensors"
fi
cd "$PROJECT_ROOT"

# ── 8. Aider ──────────────────────────────────────────────────────────────────
echo "💻 Setting up Aider..."
~/.local/bin/uv tool install --python 3.12 'aider-chat[playwright]' --force 2>/dev/null || true

# ── 9. Vault + Workspace ──────────────────────────────────────────────────────
echo "📂 Initializing directories..."
mkdir -p "$VAULT_PATH/memory" "$VAULT_PATH/images" "$VAULT_PATH/characters"
mkdir -p "$WORKSPACE_DIR"

# ── 10. Settings ──────────────────────────────────────────────────────────────
mkdir -p "$HOME/.config/horizon"
# Only write if file doesn't exist (preserve user settings on re-install)
if [ ! -f "$HOME/.config/horizon/settings.json" ]; then
    cat <<EOF > "$HOME/.config/horizon/settings.json"
{
  "vault_path": "$VAULT_PATH",
  "llm_model": "qwen2.5:14b",
  "roleplay_model": "llama3.1:8b",
  "comfyui_path": "$PROJECT_ROOT/ComfyUI/main.py",
  "embeddings_path": "$HOME/.local/share/horizon/embeddings.bin",
  "image_rating": "rating_safe",
  "agents": {
    "archivist_enabled": true,
    "vanguard_enabled": true,
    "antenna_enabled": false,
    "forge_enabled": true,
    "wiki_enabled": true,
    "antenna_token": "changeme",
    "antenna_port": 8374,
    "vanguard_interval_minutes": 30,
    "light_model": "qwen2.5:14b",
    "vanguard_feeds": [
      "https://news.ycombinator.com/rss",
      "https://feeds.feedburner.com/TheHackersNews"
    ],
    "force_agent_mode": false
  },
  "agent_workspace": "$WORKSPACE_DIR",
  "model_capabilities": {}
}
EOF
fi

# ── 11. Build ─────────────────────────────────────────────────────────────────
echo "🏗  Building Horizon (release)..."
cd "$PROJECT_ROOT/src-tauri"
cargo build --release

# ── 12. Install binary + desktop entry ───────────────────────────────────────
echo "🖥  Installing..."
BIN_DEST="$HOME/.local/bin/horizon"
ICON_DEST="$HOME/.local/share/icons/horizon.png"
DESKTOP_DEST="$HOME/.local/share/applications/horizon.desktop"

mkdir -p "$HOME/.local/bin" "$HOME/.local/share/icons" "$HOME/.local/share/applications"

cp "$PROJECT_ROOT/src-tauri/target/release/horizon" "$BIN_DEST"
chmod +x "$BIN_DEST"
cp "$PROJECT_ROOT/src-tauri/icons/icon.png" "$ICON_DEST"

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

update-desktop-database "$HOME/.local/share/applications" 2>/dev/null || true

echo ""
echo "✅ Horizon installed successfully!"
echo "   Binary : $BIN_DEST"
echo "   Launch : horizon  (or search in app menu)"
