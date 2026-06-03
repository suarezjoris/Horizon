#!/usr/bin/env bash
# Horizon v2 — Smart Auto-Repair & Full Maintenance Script

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CONFIG_FILE="$HOME/.config/horizon/settings.json"

echo "🔍 Starting Horizon Deep Maintenance..."

# 1. Check Git Updates
if [ -d ".git" ]; then
    echo "📦 Checking for updates..."
    git pull origin main --rebase || echo "⚠️ Offline or git issue, skipping update."
fi

# 2. Force Model Update (Reset config if needed)
if [ -f "$CONFIG_FILE" ]; then
    echo "⚙️ Updating configuration to new standards..."
    # Force qwen2.5-coder:14b if it's still on old model
    sed -i 's/dolphin-mixtral:8x7b/qwen2.5-coder:14b/g' "$CONFIG_FILE"
    sed -i 's/dolphin-mixtral/qwen2.5-coder:14b/g' "$CONFIG_FILE"
else
    echo "⚠️ Config missing! Creating default settings..."
    mkdir -p "$HOME/.config/horizon"
    echo "{\"vault_path\": \"$HOME/Documents/Claude RAG\", \"llm_model\": \"qwen2.5-coder:14b\", \"roleplay_model\": \"llama3.1:8b\", \"comfyui_path\": \"$PROJECT_ROOT/ComfyUI/main.py\", \"embeddings_path\": \"$HOME/.local/share/horizon/embeddings.bin\", \"image_rating\": \"rating_safe\"}" > "$CONFIG_FILE"
fi

# 3. Smart Search for ComfyUI if broken
CURRENT_COMFY=$(grep -oP '(?<="comfyui_path": ")[^"]*' "$CONFIG_FILE")
if [ ! -f "$CURRENT_COMFY" ]; then
    echo "⚠️ ComfyUI not found at $CURRENT_COMFY"
    echo "🔍 Searching for ComfyUI/main.py (excluding venv)..."
    SEARCH_PATH=$(find "$HOME/Projects" -name "main.py" -not -path "*/venv/*" -not -path "*/site-packages/*" | grep "ComfyUI" | head -n 1)
    
    if [ -n "$SEARCH_PATH" ]; then
        echo "✅ Found ComfyUI at $SEARCH_PATH. Updating config..."
        sed -i "s|\"comfyui_path\": \"[^\"]*\"|\"comfyui_path\": \"$SEARCH_PATH\"|g" "$CONFIG_FILE"
    fi
fi

# 4. Verify Ollama Models
echo "🧠 Checking Ollama models..."
if ollama list | grep -q "qwen2.5-coder:14b"; then
    echo "✅ qwen2.5-coder:14b found."
    ollama cp qwen2.5-coder:14b gpt-4o || true
else
    echo "📥 Pulling qwen2.5-coder:14b (Essential for performance)..."
    ollama pull qwen2.5-coder:14b
    ollama cp qwen2.5-coder:14b gpt-4o || true
fi

# 5. REBUILD APPLICATION (Crucial to apply UI and code fixes)
echo "🏗️ Rebuilding Horizon to apply UI and Architecture fixes..."
cd "$PROJECT_ROOT/src-tauri"
source "$HOME/.cargo/env" || true
# Run build - this ensures the NEW CSS and HTML are baked into the binary
cargo tauri build

# 6. Install new Binary and System Entry
echo "🖥️ Re-installing binary and system integration..."
BIN_DEST="$HOME/.local/bin/horizon"
ICON_DEST="$HOME/.local/share/icons/horizon.png"
DESKTOP_DEST="$HOME/.local/share/applications/horizon.desktop"

mkdir -p "$HOME/.local/bin"
cp "$PROJECT_ROOT/src-tauri/target/release/horizon" "$BIN_DEST"
chmod +x "$BIN_DEST"

mkdir -p "$HOME/.local/share/icons"
cp "$PROJECT_ROOT/src-tauri/icons/icon.png" "$ICON_DEST"

cat <<EOL > "$DESKTOP_DEST"
[Desktop Entry]
Name=Horizon
Comment=Personal AI Assistant
Exec=$BIN_DEST
Icon=$ICON_DEST
Terminal=false
Type=Application
Categories=Development;Utility;
EOL

update-desktop-database "$HOME/.local/share/applications" || true

echo ""
echo "✅ Horizon is fully repaired, compiled, and up to date."
echo "Launch it from your application menu or type 'horizon' in terminal."
