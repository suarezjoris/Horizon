#!/usr/bin/env bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
VAULT="$HOME/Documents/Claude RAG"
VENV="$HOME/.personal-ai/venv"

echo "=== Personal AI — Installation ==="

# 1. Vérifier Ollama
if ! ollama list > /dev/null 2>&1; then
    echo "ERREUR : Ollama ne répond pas. Lance: systemctl --user start ollama"
    exit 1
fi

# 2. Pull nomic-embed-text
echo "[1/5] Pull nomic-embed-text (modèle d'embeddings)..."
ollama pull nomic-embed-text

# 3. Venv + dépendances
echo "[2/5] Création du venv Python..."
python3 -m venv "$VENV"
source "$VENV/bin/activate"
pip install -q --upgrade pip
pip install -q -r "$SCRIPT_DIR/requirements.txt"

# 4. Vault
echo "[3/5] Initialisation de la vault Obsidian..."
mkdir -p "$VAULT/memory" "$VAULT/conversations" "$VAULT/knowledge"

if [ ! -f "$VAULT/memory/user.md" ]; then
cat > "$VAULT/memory/user.md" << 'EOF'
# User
- Prénom : Joris
- Système : Arch Linux, Hyprland
- Centres d'intérêt : développement, ethical hacking, automatisation
- Langages : Go, Python
EOF
fi

if [ ! -f "$VAULT/memory/code.md" ]; then
cat > "$VAULT/memory/code.md" << 'EOF'
# Code
- Style : simple, chirurgical, pas d'over-engineering
- Langages préférés : [[Go]], [[Python]]
- Outils : [[Hyprland]], systemd, Neovim
- Ethique : ethical hacking, CTF, sécurité défensive
EOF
fi

if [ ! -f "$VAULT/memory/skills.md" ]; then
cat > "$VAULT/memory/skills.md" << 'EOF'
# Skills
- [[code]] : développement Go, Python, automatisation shell
- Ethical hacking : recon, exploitation, CTF
- DevOps : systemd, Arch Linux, Docker
EOF
fi

# 5. Wrapper script
echo "[4/5] Création du lanceur..."
mkdir -p "$HOME/.local/bin"
cat > "$HOME/.local/bin/personal-ai" << EOF
#!/usr/bin/env bash
source "$VENV/bin/activate"
cd "$SCRIPT_DIR"
python main.py
EOF
chmod +x "$HOME/.local/bin/personal-ai"

# Alias zsh
if ! grep -q "alias ai=" "$HOME/.zshrc" 2>/dev/null; then
    echo "alias ai='personal-ai'" >> "$HOME/.zshrc"
fi

# 6. Keybind Hyprland
echo "[5/5] Keybind Hyprland (SUPER+C)..."
KEYBINDS="$HOME/.config/hypr/configs/Keybinds.conf"
if [ -f "$KEYBINDS" ] && ! grep -q "personal-ai" "$KEYBINDS"; then
    cat >> "$KEYBINDS" << 'EOF'

# Personal AI (floating terminal)
bind = SUPER, C, exec, kitty --class=personal-ai --title="Personal AI" personal-ai
EOF
fi

# Règle fenêtre flottante Hyprland
HYPR_CONF="$HOME/.config/hypr/hyprland.conf"
if [ -f "$HYPR_CONF" ] && ! grep -q "personal-ai" "$HYPR_CONF"; then
    cat >> "$HYPR_CONF" << 'EOF'

# Personal AI floating window
windowrule = match:class personal-ai, float on, size 900 600, center on
EOF
fi

echo ""
echo "=== Installation terminée ==="
echo "Commandes disponibles :"
echo "  ai              → lance l'IA (après source ~/.zshrc)"
echo "  personal-ai     → même chose"
echo "  SUPER+C         → fenêtre flottante Hyprland"
echo ""
echo "Lance maintenant : source ~/.zshrc && ai"
