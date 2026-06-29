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
    # Arch, Manjaro, EndeavourOS, Garuda…
    sudo pacman -Sy --needed --noconfirm \
        webkit2gtk-4.1 base-devel curl wget git bubblewrap \
        openblas nspr nss at-spi2-core libdrm mesa \
        libxcomposite libxdamage libxfixes libxrandr alsa-lib pango cairo \
        python python-pip
elif command -v apt-get &>/dev/null; then
    # Debian, Ubuntu, Mint, Pop!_OS, elementary, Kali…
    sudo apt-get update -qq
    sudo apt-get install -y \
        build-essential curl wget git bubblewrap \
        libssl-dev libgtk-3-dev librsvg2-dev \
        python3 python3-pip python3-venv
    # webkit/soup/jsc: try 4.1 (newer), fall back to 4.0 (older Ubuntu/Debian)
    sudo apt-get install -y libwebkit2gtk-4.1-dev libjavascriptcoregtk-4.1-dev libsoup-3.0-dev \
        || sudo apt-get install -y libwebkit2gtk-4.0-dev libjavascriptcoregtk-4.0-dev libsoup2.4-dev
elif command -v dnf &>/dev/null; then
    # Fedora, RHEL 8+, Rocky, Alma, CentOS Stream…
    sudo dnf install -y \
        webkit2gtk4.1-devel gcc make curl wget git bubblewrap \
        openssl-devel gtk3-devel libsoup3-devel \
        python3 python3-pip \
        || sudo dnf install -y \
        webkit2gtk3-devel gcc make curl wget git bubblewrap \
        openssl-devel gtk3-devel libsoup-devel python3 python3-pip
elif command -v zypper &>/dev/null; then
    # openSUSE Leap / Tumbleweed
    sudo zypper install -y \
        webkit2gtk3-devel gcc make curl wget git bubblewrap \
        libopenssl-devel gtk3-devel libsoup3-devel \
        python3 python3-pip
elif command -v yum &>/dev/null; then
    # Older RHEL / CentOS 7
    sudo yum install -y \
        webkit2gtk3-devel gcc make curl wget git bubblewrap \
        openssl-devel gtk3-devel libsoup-devel python3 python3-pip
elif command -v apk &>/dev/null; then
    # Alpine (musl — best effort)
    sudo apk add --no-cache \
        webkit2gtk-dev build-base curl wget git bubblewrap \
        openssl-dev gtk+3.0-dev libsoup3-dev librsvg-dev \
        python3 py3-pip
elif command -v xbps-install &>/dev/null; then
    # Void Linux
    sudo xbps-install -Sy \
        webkit2gtk-devel base-devel curl wget git bubblewrap \
        openssl-devel gtk+3-devel libsoup3-devel librsvg-devel \
        python3 python3-pip
elif command -v eopkg &>/dev/null; then
    # Solus
    sudo eopkg install -y -c system.devel
    sudo eopkg install -y \
        libwebkit-gtk-devel curl wget git bubblewrap \
        openssl-devel libgtk-3-devel libsoup-devel librsvg-devel \
        python3
else
    echo "⚠️  Unsupported package manager. Install these dependencies manually, then re-run:"
    echo "    • WebKitGTK 4.1 (or 4.0) dev headers + libsoup3 (or 2.4) + javascriptcoregtk dev"
    echo "    • GTK3 dev, librsvg dev, OpenSSL dev"
    echo "    • a C toolchain (gcc/make), curl, wget, git, bubblewrap, python3 + pip"
    echo "    (Gentoo: emerge the equivalents; NixOS: a shell with these in buildInputs.)"
    read -p "Press Enter once dependencies are installed to continue, or Ctrl-C to abort... " _
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
~/.local/bin/uv pip install -q --upgrade \
    --python "$PROJECT_ROOT/.venv/bin/python3" \
    ddgs readability-lxml requests mcp 2>/dev/null \
    || "$PROJECT_ROOT/.venv/bin/python3" -m pip install -q --upgrade \
        ddgs readability-lxml requests mcp

# ── 6. Ollama & Models ────────────────────────────────────────────────────────
echo "🧠 Setting up Ollama..."
if ! command -v ollama &>/dev/null; then
    echo "📥 Installing Ollama..."
    curl -fsSL https://ollama.com/install.sh | sh
fi

echo "📥 Pulling models (this may take a while)..."
ollama pull qwen2.5-coder:14b
ollama pull qwen2.5-coder:32b
ollama pull llama3.1:8b
ollama pull nomic-embed-text:latest
ollama pull moondream:latest

# ── 6b. Ollama VRAM Optimization ─────────────────────────────────────────────
echo "⚡ Optimizing Ollama for VRAM efficiency..."
sudo mkdir -p /etc/systemd/system/ollama.service.d
printf '[Service]\nEnvironment="OLLAMA_KV_CACHE_TYPE=q4_0"\nEnvironment="OLLAMA_FLASH_ATTENTION=1"\nEnvironment="OLLAMA_NUM_PARALLEL=1"\n' \
    | sudo tee /etc/systemd/system/ollama.service.d/optimize.conf > /dev/null
sudo systemctl daemon-reload
sudo systemctl restart ollama

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
    ~/.local/bin/uv pip install --python venv/bin/python3 -r requirements.txt -q 2>/dev/null \
        || venv/bin/python3 -m pip install -r requirements.txt -q
    ~/.local/bin/uv pip install --python venv/bin/python3 opencv-python-headless imageio-ffmpeg -q 2>/dev/null \
        || venv/bin/python3 -m pip install opencv-python-headless imageio-ffmpeg -q
fi

# Always ensure extra deps are present (runs even if venv already existed)
~/.local/bin/uv pip install --python venv/bin/python3 sqlalchemy -q 2>/dev/null \
    || venv/bin/python3 -m pip install sqlalchemy -q

mkdir -p "$MODELS_DIR"
if [ ! -f "$MODELS_DIR/ponyDiffusionV6XL_v6.safetensors" ]; then
    echo "📥 Downloading Pony Diffusion V6 XL (6.5 GB)..."
    wget --show-progress \
        -O "$MODELS_DIR/ponyDiffusionV6XL_v6.safetensors" \
        "https://huggingface.co/LyliaEngine/Pony_Diffusion_V6_XL/resolve/main/ponyDiffusionV6XL_v6StartWithThis.safetensors" \
        || { rm -f "$MODELS_DIR/ponyDiffusionV6XL_v6.safetensors"; \
             echo "⚠️  Model download failed. Download manually to:"; \
             echo "   $MODELS_DIR/ponyDiffusionV6XL_v6.safetensors"; }
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
  "llm_model": "qwen2.5-coder:14b",
  "heavy_model": "qwen2.5-coder:32b",
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
    "light_model": "qwen2.5-coder:14b",
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
