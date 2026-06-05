#!/usr/bin/env bash
# Horizon v2 — Smart Auto-Repair & Full Maintenance Script
set -e

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CONFIG_FILE="$HOME/.config/horizon/settings.json"

echo "🔍 Starting Horizon Deep Maintenance..."

# 1. Check Git Updates
if [ -d ".git" ]; then
    echo "📦 Checking for updates..."
    # Only stash if there are local changes, so we never pop an unrelated stash.
    STASHED=0
    if ! git diff --quiet || ! git diff --cached --quiet; then
        git stash push -m "Horizon Auto-Update Stash" > /dev/null 2>&1 && STASHED=1
    fi
    git pull origin main --rebase || echo "⚠️ Offline, skipping pull."
    # Restore local changes. If pop conflicts, FAIL LOUDLY instead of silently
    # dropping the work into the stash (this is what caused lost edits before).
    if [ "$STASHED" = "1" ]; then
        if ! git stash pop; then
            echo "❌ 'git stash pop' failed — your local changes are SAFE in 'git stash list'."
            echo "   Resolve the conflict manually, then re-run ./update.sh. Aborting."
            exit 1
        fi
    fi
fi

# 2. Fix ComfyUI Dependencies
echo "🛠️ Repairing ComfyUI Dependencies..."
cd "$PROJECT_ROOT/ComfyUI"
./venv/bin/python3 -m pip install opencv-python-headless imageio-ffmpeg > /dev/null 2>&1

# 3. Force Model Update
if [ -f "$CONFIG_FILE" ]; then
    sed -i 's/dolphin-mixtral:8x7b/qwen2.5-coder:14b/g' "$CONFIG_FILE"
    sed -i 's/dolphin-mixtral/qwen2.5-coder:14b/g' "$CONFIG_FILE"
fi

# 4. REBUILD APPLICATION
echo "🏗️ Rebuilding Horizon..."
cd "$PROJECT_ROOT/src-tauri"
source "$HOME/.cargo/env" || true
cargo build --release

# 5. Install
BIN_DEST="$HOME/.local/bin/horizon"
mkdir -p "$HOME/.local/bin"
cp "$PROJECT_ROOT/src-tauri/target/release/horizon" "$BIN_DEST"
chmod +x "$BIN_DEST"

echo "✅ Horizon is fully repaired and compiled."
