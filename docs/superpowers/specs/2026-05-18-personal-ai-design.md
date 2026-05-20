**Personal AI — Design Spec**  
**Date:** 2026-05-18  
   
 **Stack:** Python 3.11 · Textual · ChromaDB · Ollama (dolphin-mixtral:8x7b) · nomic-embed-text  
![](data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAnEAAAACCAYAAAA3pIp+AAAABmJLR0QA/wD/AP+gvaeTAAAACXBIWXMAAA7EAAAOxAGVKw4bAAAANklEQVR4nO3OQQmAABRAsSeYxZy/lHd7GMACBrCCNxG2BFtmZquOAAD4i3Ot7mr/egIAwGvXA7GTBde8bLBeAAAAAElFTkSuQmCC)  
**Objectif**  
IA personnelle locale, sans filtre, usage quotidien. Mémoire persistante via vault Obsidian (~/Documents/Claude RAG/). Accès internet autonome. Interface TUI flottante dans Hyprland.  
![](data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAnEAAAACCAYAAAA3pIp+AAAABmJLR0QA/wD/AP+gvaeTAAAACXBIWXMAAA7EAAAOxAGVKw4bAAAANUlEQVR4nO3OMQ2AABAAsSNBCkJfE1pYGfHAiAU2QtIq6DIzW7UHAMBfnGt1V8fXEwAAXrse4dwF6o2O55YAAAAASUVORK5CYII=)  
**Architecture**  
~/Projects/personal-ai/  
 ├── main.py              # point d'entrée  
 ├── config.py            # constantes (chemins, modèles, ports)  
 ├── install.sh           # installation one-shot  
 ├── agent/  
 │   ├── core.py          # boucle agent : reçoit message, orchestre, retourne réponse  
 │   ├── memory.py        # RAG : embed query → ChromaDB → graph traversal → inject context  
 │   └── tools.py         # web_search(query), fetch_url(url)  
 ├── tui/  
 │   └── app.py           # Textual : chat streamé, keybinds, commandes /  
 └── vault/  
     ├── manager.py       # lire/écrire/lister les .md dans Claude RAG/  
     └── indexer.py       # embed tous les .md → ChromaDB (startup + watch)  
   
![](data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAnEAAAACCAYAAAA3pIp+AAAABmJLR0QA/wD/AP+gvaeTAAAACXBIWXMAAA7EAAAOxAGVKw4bAAAANElEQVR4nO3OQQmAUBBAwSd8bOHVnBvBkAaxgjcRZhLMNjNHdQUAwF/cq9qr8+sJAACvrQctgQNH4A++9QAAAABJRU5ErkJggg==)  
**Flux d'une conversation**  
1. User tape un message dans le TUI  
 2. agent/memory.py : embed le message → recherche ChromaDB → top-3 notes  
 3. Pour chaque note trouvée : suit les liens [[wikilink]] Obsidian (1 niveau)  
 4. Contexte assemblé → injecté dans le system prompt  
 5. agent/core.py : appelle dolphin-mixtral via Ollama API (streaming)  
 6. TUI affiche la réponse en temps réel  
 7. Post-réponse : l'IA reçoit un second prompt "extract memory facts"  
 8. Elle retourne un JSON {path, content} → vault/manager.py écrit/met à jour le .md  
 9. vault/indexer.py re-indexe les notes modifiées  
   
![](data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAnEAAAACCAYAAAA3pIp+AAAABmJLR0QA/wD/AP+gvaeTAAAACXBIWXMAAA7EAAAOxAGVKw4bAAAANklEQVR4nO3OQQmAABRAsScYxpg/h5VMYARvRrCCNxG2BFtmZquOAAD4i3Ot7mr/egIAwGvXA224BcUMk6pDAAAAAElFTkSuQmCC)  
**Structure de la Vault**  
~/Documents/Claude RAG/  
 ├── memory/  
 │   ├── user.md          # identité, préférences, contexte perso  
 │   ├── projects.md      # projets actifs avec statut  
 │   ├── code.md          # langages, style, outils préférés  
 │   └── skills.md        # domaines maîtrisés (ethical hacking, Go, Python...)  
 ├── conversations/  
 │   └── YYYY-MM-DD.md    # log quotidien (append-only)  
 └── knowledge/  
     └── *.md             # notes auto-générées par l'IA sur des sujets  
   
Les fichiers memory/ sont pré-remplis manuellement au démarrage.  
   
 knowledge/ et conversations/ sont 100% gérés par l'IA.  
![](data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAnEAAAACCAYAAAA3pIp+AAAABmJLR0QA/wD/AP+gvaeTAAAACXBIWXMAAA7EAAAOxAGVKw4bAAAANklEQVR4nO3OMQ2AABAAsSNBCUpfDq4wwIAABiywEZJWQZeZ2ao9AAD+4liruzq/ngAA8Nr1ABweBgdur/QFAAAAAElFTkSuQmCC)  
**Mémoire Vectorielle (RAG)**  
- **Moteur :** ChromaDB local, persiste dans ~/.personal-ai/chroma/  
- **Embeddings :**nomic-embed-text via Ollama (274MB, local, pas de GPU requis)  
- **Indexation :** au démarrage, tous les .md de la vault sont indexés par chunk (512 tokens, overlap 64)  
- **Recherche :** cosine similarity, top-3 résultats, seuil minimum 0.4  
- **Graph traversal :** pour chaque résultat, parse les [[liens]] et charge les notes liées (profondeur 1)  
- **Injection :** le contexte assemblé est injecté en <memory> dans le system prompt  
![](data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAnEAAAACCAYAAAA3pIp+AAAABmJLR0QA/wD/AP+gvaeTAAAACXBIWXMAAA7EAAAOxAGVKw4bAAAAM0lEQVR4nO3OQQmAUBBAwSeILbyYdDP8jAaxgjcRZhLMNjNntQIA4C/uvTqq6+sJAADvPS2NA0FrXqf/AAAAAElFTkSuQmCC)  
**Outils Internet**  
L'IA dispose de 2 outils via **ReAct prompt** (pas de tool calling natif Ollama — trop instable selon les modèles) :  
| | | |  
|-|-|-|  
| **Outil** | **Lib** | **Comportement** |   
| web_search(query, n=5) | duckduckgo-search | Retourne titre + URL + snippet des n premiers résultats |   
| fetch_url(url) | httpx + html2text | Charge la page, extrait le texte brut (max 8000 chars) |   
   
**Mécanisme ReAct :** le system prompt décrit les outils. L'IA répond avec ACTION: web_search("..."). agent/core.py parse la réponse, exécute l'outil, injecte le résultat, et relance la génération. Boucle jusqu'à une réponse finale sans ACTION.  
L'IA décide seule quand les utiliser. Elle peut chaîner les deux (search → fetch).  
![](data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAnEAAAACCAYAAAA3pIp+AAAABmJLR0QA/wD/AP+gvaeTAAAACXBIWXMAAA7EAAAOxAGVKw4bAAAANklEQVR4nO3OQQmAABRAsScYxpg/h5VMYARvRrCCNxG2BFtmZquOAAD4i3Ot7mr/egIAwGvXA224BcUMk6pDAAAAAElFTkSuQmCC)  
**TUI (Textual)**  
- Fenêtre de chat avec streaming token par token  
- Panneau gauche : historique de session  
- Zone principale : conversation avec markdown rendu  
- Indicateur animé pendant la génération  
- Commandes disponibles :  
- /clear — réinitialise la conversation  
- /memory — affiche les notes chargées pour le dernier message  
- /search <query> — recherche manuelle dans la vault  
- /note <texte> — force l'écriture d'une note dans la vault  
![](data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAnEAAAACCAYAAAA3pIp+AAAABmJLR0QA/wD/AP+gvaeTAAAACXBIWXMAAA7EAAAOxAGVKw4bAAAANUlEQVR4nO3OMQ2AABAAsSPBCUZfEnoYmFDBhAU2QtIq6DIzW7UHAMBfnGt1V8fXEwAAXrse/wcF74lXkIsAAAAASUVORK5CYII=)  
**Installation**  
install.sh fait dans l'ordre :  
1. Vérifie que Ollama tourne (ollama list)  
2. Pull nomic-embed-text via Ollama  
3. Crée un venv Python dans ~/.personal-ai/venv/  
4. Installe les dépendances (pip install -r requirements.txt)  
5. Initialise la vault ~/Documents/Claude RAG/ avec les fichiers memory/ de base  
6. Ajoute un keybind dans ~/.config/hypr/Keybinds.conf : SUPER+C → fenêtre flottante  
7. Crée un alias ai dans .zshrc  
![](data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAnEAAAACCAYAAAA3pIp+AAAABmJLR0QA/wD/AP+gvaeTAAAACXBIWXMAAA7EAAAOxAGVKw4bAAAANUlEQVR4nO3OMQ2AUBBAsUfyRTCh9VRgEBGsWGAjJK2CbjNzVGcAAPzFtapV7V9PAAB47X4AEWgEMAY9+pUAAAAASUVORK5CYII=)  
**Dépendances Python**  
textual>=0.59  
 chromadb>=0.5  
 httpx>=0.27  
 duckduckgo-search>=6.0  
 html2text>=2024.2  
 ollama>=0.2  
 watchdog>=4.0  
   
![](data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAnEAAAACCAYAAAA3pIp+AAAABmJLR0QA/wD/AP+gvaeTAAAACXBIWXMAAA7EAAAOxAGVKw4bAAAANUlEQVR4nO3OQQmAABRAsSd4NIGhrOTvaQBrWMGbCFuCLTOzV2cAAPzFvVZbdXw9AQDgtesBhYQEO+64Y8AAAAAASUVORK5CYII=)  
**Configuration (**config.py **)**  
VAULT_PATH = "~/Documents/Claude RAG"  
 CHROMA_PATH = "~/.personal-ai/chroma"  
 MODEL = "dolphin-mixtral:8x7b"  
 EMBED_MODEL = "nomic-embed-text"  
 OLLAMA_HOST = "http://localhost:11434"  
 CHUNK_SIZE = 512  
 CHUNK_OVERLAP = 64  
 TOP_K = 3  
 SIMILARITY_THRESHOLD = 0.4  
   
![](data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAnEAAAACCAYAAAA3pIp+AAAABmJLR0QA/wD/AP+gvaeTAAAACXBIWXMAAA7EAAAOxAGVKw4bAAAANklEQVR4nO3OMQ2AABAAsSPBCj5fFgpQwYwEZiywEZJWQZeZ2ao9AAD+4lyruzq+ngAA8Nr1AMTRBeEgNK9YAAAAAElFTkSuQmCC)  
**Contraintes**  
- Tout tourne local, aucune donnée ne sort de la machine  
- Pas de clé API externe requise  
- Fonctionne sans GPU pour les embeddings (nomic-embed-text est CPU-friendly)  
- dolphin-mixtral utilise VRAM + RAM overflow géré par Ollama  
