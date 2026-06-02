#!/usr/bin/env bash
set -e

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN_DEST="$HOME/.local/bin/horizon"

echo "🏗️  Compilation de Horizon v2..."
cd "$PROJECT_ROOT/src-tauri"
source "$HOME/.cargo/env" || true
cargo tauri build

echo "🚚 Déploiement vers $BIN_DEST..."
cp "$PROJECT_ROOT/src-tauri/target/release/horizon" "$BIN_DEST"
chmod +x "$BIN_DEST"

echo "🖼️ Mise à jour de l'icône..."
mkdir -p "$HOME/.local/share/icons"
cp "$PROJECT_ROOT/src-tauri/icons/icon.png" "$HOME/.local/share/icons/horizon.png"

echo "📝 Mise à jour du raccourci application..."
DESKTOP_DEST="$HOME/.local/share/applications/horizon.desktop"
cat <<EOF > "$DESKTOP_DEST"
[Desktop Entry]
Name=Horizon
Comment=Personal AI Assistant (LLM, Image, Roleplay, Code)
Exec=$BIN_DEST
Icon=$HOME/.local/share/icons/horizon.png
Terminal=false
Type=Application
Categories=Development;Utility;
Keywords=ai;llm;chat;code;
EOF

echo "🔄 Mise à jour du menu système..."
update-desktop-database "$HOME/.local/share/applications" || true

echo "✅ Horizon est à jour et prêt à être lancé !"
