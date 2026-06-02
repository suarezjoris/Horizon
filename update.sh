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

echo "🔄 Mise à jour du menu système..."
update-desktop-database "$HOME/.local/share/applications" || true

echo "✅ Horizon est à jour et prêt à être lancé !"
