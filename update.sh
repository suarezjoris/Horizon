#!/usr/bin/env bash
# Horizon — Smart Update & Repair Script
set -e

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CONFIG_FILE="$HOME/.config/horizon/settings.json"

echo "🔍 Starting Horizon Maintenance..."

# ── 1. Git update ─────────────────────────────────────────────────────────────
if [ -d ".git" ]; then
    echo "📦 Checking for updates..."
    STASHED=0
    if ! git diff --quiet || ! git diff --cached --quiet; then
        git stash push -m "Horizon Auto-Update Stash" >/dev/null 2>&1 && STASHED=1
    fi
    git pull origin main --rebase || echo "⚠️  Offline, skipping pull."
    if [ "$STASHED" = "1" ]; then
        if ! git stash pop; then
            echo "❌ 'git stash pop' failed — changes are safe in 'git stash list'."
            echo "   Resolve the conflict manually, then re-run ./update.sh. Aborting."
            exit 1
        fi
    fi
fi

# ── 2. Search venv deps ───────────────────────────────────────────────────────
if [ ! -d "$PROJECT_ROOT/.venv" ]; then
    echo "🐍 Creating search venv..."
    ~/.local/bin/uv venv --python 3.12 "$PROJECT_ROOT/.venv" 2>/dev/null \
        || python3 -m venv "$PROJECT_ROOT/.venv"
fi
~/.local/bin/uv pip install -q --upgrade \
    --python "$PROJECT_ROOT/.venv/bin/python3" \
    ddgs readability-lxml requests 2>/dev/null \
    || "$PROJECT_ROOT/.venv/bin/python3" -m pip install -q --upgrade \
        ddgs readability-lxml requests

# ── 3. ComfyUI deps (only if venv exists) ────────────────────────────────────
if [ -d "$PROJECT_ROOT/ComfyUI/venv" ]; then
    echo "🛠  Repairing ComfyUI dependencies..."
    "$PROJECT_ROOT/ComfyUI/venv/bin/python3" -m pip install -q \
        opencv-python-headless imageio-ffmpeg
fi

# ── 4. Migrate settings to V4 (preserve existing values) ─────────────────────
if [ -f "$CONFIG_FILE" ]; then
    echo "⚙️  Migrating settings..."
    python3 - <<'PYEOF'
import json, os, sys

cfg = os.path.expanduser("~/.config/horizon/settings.json")
project = os.path.dirname(os.path.realpath(__file__))

with open(cfg) as f:
    s = json.load(f)

changed = False

# Remove legacy model references
LEGACY_MODELS = ("dolphin-mixtral:8x7b", "dolphin-mixtral", "qwen2.5-coder:14b")
for key in ("llm_model", "roleplay_model"):
    if s.get(key) in LEGACY_MODELS:
        s[key] = "qwen2.5:14b"
        changed = True

if s.get("agents", {}).get("light_model") in LEGACY_MODELS:
    s["agents"]["light_model"] = "qwen2.5:14b"
    changed = True

# Add V4 fields if missing
if "agent_workspace" not in s:
    ws = os.path.join(project, "workspace")
    os.makedirs(ws, exist_ok=True)
    s["agent_workspace"] = ws
    changed = True

if "model_capabilities" not in s:
    s["model_capabilities"] = {}
    changed = True

if "force_agent_mode" not in s.get("agents", {}):
    s.setdefault("agents", {})["force_agent_mode"] = False
    changed = True

if changed:
    with open(cfg, "w") as f:
        json.dump(s, f, indent=2)
    print("   Settings updated.")
else:
    print("   Settings already up to date.")
PYEOF
fi

# ── 5. Rebuild ────────────────────────────────────────────────────────────────
echo "🏗  Rebuilding Horizon..."
cd "$PROJECT_ROOT/src-tauri"
source "$HOME/.cargo/env" || true
cargo build --release

# ── 6. Install binary ─────────────────────────────────────────────────────────
BIN_DEST="$HOME/.local/bin/horizon"
mkdir -p "$HOME/.local/bin"
cp "$PROJECT_ROOT/src-tauri/target/release/horizon" "$BIN_DEST"
chmod +x "$BIN_DEST"

echo "✅ Horizon updated and compiled."
