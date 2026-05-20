# Personal AI — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** IA personnelle locale avec mémoire persistante (vault Obsidian + ChromaDB RAG), accès internet (DuckDuckGo + URL fetch), TUI Textual flottant dans Hyprland.

**Architecture:** Python app en 5 modules (config, vault, agent, tui, main). Ollama gère LLM + embeddings. ChromaDB stocke les vecteurs. La vault lit/écrit des .md dans `~/Documents/Claude RAG/`. Le loop ReAct gère les outils. Tests avec mocks pour toutes les dépendances externes.

**Tech Stack:** Python 3.11 · Textual≥0.59 · ChromaDB≥0.5 · ollama-python≥0.2 · duckduckgo-search≥6.0 · httpx≥0.27 · html2text≥2024.2 · pytest≥8.0 · pytest-mock≥3.12

---

## Fichiers créés

```
personal-ai/
├── requirements.txt
├── config.py
├── main.py
├── install.sh
├── agent/
│   ├── __init__.py
│   ├── core.py          # loop ReAct + save memory post-réponse
│   ├── memory.py        # RAG : embed → ChromaDB → graph traversal → contexte
│   └── tools.py         # web_search(), fetch_url()
├── tui/
│   ├── __init__.py
│   └── app.py           # Textual : chat streamé, commandes /
├── vault/
│   ├── __init__.py
│   ├── manager.py       # lire/écrire/lister .md
│   └── indexer.py       # embed + ChromaDB (index au démarrage + après écriture)
└── tests/
    ├── __init__.py
    ├── conftest.py
    ├── test_manager.py
    ├── test_indexer.py
    ├── test_memory.py
    ├── test_tools.py
    └── test_core.py
```

---

### Task 1 : Setup projet

**Files:**
- Create: `requirements.txt`
- Create: `config.py`
- Create: `tests/__init__.py`
- Create: `tests/conftest.py`
- Create: `agent/__init__.py`, `tui/__init__.py`, `vault/__init__.py`

- [ ] **Step 1 : Créer requirements.txt**

```
textual>=0.59
chromadb>=0.5
httpx>=0.27
duckduckgo-search>=6.0
html2text>=2024.2
ollama>=0.2
pytest>=8.0
pytest-mock>=3.12
```

- [ ] **Step 2 : Créer config.py**

```python
from pathlib import Path

VAULT_PATH = Path("~/Documents/Claude RAG").expanduser()
CHROMA_PATH = Path("~/.personal-ai/chroma").expanduser()
MODEL = "dolphin-mixtral:8x7b"
EMBED_MODEL = "nomic-embed-text"
OLLAMA_HOST = "http://localhost:11434"
CHUNK_SIZE = 512
CHUNK_OVERLAP = 64
TOP_K = 3
SIMILARITY_THRESHOLD = 0.4
```

- [ ] **Step 3 : Créer les __init__.py et conftest.py**

```bash
touch agent/__init__.py tui/__init__.py vault/__init__.py tests/__init__.py
```

```python
# tests/conftest.py
import pytest
from pathlib import Path

@pytest.fixture
def tmp_vault(tmp_path):
    """Vault temporaire pour les tests."""
    vault = tmp_path / "vault"
    vault.mkdir()
    (vault / "memory").mkdir()
    (vault / "knowledge").mkdir()
    (vault / "conversations").mkdir()
    return vault
```

- [ ] **Step 4 : Créer le venv et installer les dépendances**

```bash
python3 -m venv ~/.personal-ai/venv
source ~/.personal-ai/venv/bin/activate
pip install -r requirements.txt
```

Attendu : installation sans erreur.

- [ ] **Step 5 : Vérifier que pytest tourne**

```bash
source ~/.personal-ai/venv/bin/activate
pytest --collect-only
```

Attendu : `no tests ran` (0 erreurs).

- [ ] **Step 6 : Commit**

```bash
git add requirements.txt config.py agent/__init__.py tui/__init__.py vault/__init__.py tests/__init__.py tests/conftest.py
git commit -m "feat: project setup, config, venv"
```

---

### Task 2 : Vault Manager

**Files:**
- Create: `vault/manager.py`
- Create: `tests/test_manager.py`

- [ ] **Step 1 : Écrire les tests**

```python
# tests/test_manager.py
import pytest
from pathlib import Path
from unittest.mock import patch

def test_write_and_read_note(tmp_vault):
    with patch("vault.manager.VAULT_PATH", tmp_vault):
        from vault.manager import write_note, read_note
        write_note("memory/user.md", "# User\n- Joris")
        assert read_note("memory/user.md") == "# User\n- Joris"

def test_write_creates_parent_dirs(tmp_vault):
    with patch("vault.manager.VAULT_PATH", tmp_vault):
        from vault.manager import write_note, read_note
        write_note("knowledge/python/tips.md", "content")
        assert read_note("knowledge/python/tips.md") == "content"

def test_read_missing_note_raises(tmp_vault):
    with patch("vault.manager.VAULT_PATH", tmp_vault):
        from vault.manager import read_note
        with pytest.raises(FileNotFoundError):
            read_note("missing.md")

def test_list_notes(tmp_vault):
    with patch("vault.manager.VAULT_PATH", tmp_vault):
        from vault.manager import write_note, list_notes
        write_note("memory/user.md", "a")
        write_note("knowledge/python.md", "b")
        paths = list_notes()
        assert len(paths) == 2

def test_extract_wikilinks():
    from vault.manager import extract_wikilinks
    content = "Voir [[code]] et [[python]] pour plus d'infos. Aussi [[user]]."
    links = extract_wikilinks(content)
    assert links == ["code", "python", "user"]

def test_extract_wikilinks_empty():
    from vault.manager import extract_wikilinks
    assert extract_wikilinks("Pas de liens ici.") == []

def test_append_to_note(tmp_vault):
    with patch("vault.manager.VAULT_PATH", tmp_vault):
        from vault.manager import write_note, append_note, read_note
        write_note("memory/user.md", "# User")
        append_note("memory/user.md", "- nouveau fait")
        assert read_note("memory/user.md") == "# User\n- nouveau fait"

def test_append_creates_if_missing(tmp_vault):
    with patch("vault.manager.VAULT_PATH", tmp_vault):
        from vault.manager import append_note, read_note
        append_note("memory/new.md", "- premier fait")
        assert read_note("memory/new.md") == "- premier fait"
```

- [ ] **Step 2 : Vérifier que les tests échouent**

```bash
source ~/.personal-ai/venv/bin/activate
pytest tests/test_manager.py -v
```

Attendu : `ERROR` ou `ImportError` (vault/manager.py n'existe pas encore).

- [ ] **Step 3 : Implémenter vault/manager.py**

```python
# vault/manager.py
import re
from pathlib import Path
from config import VAULT_PATH

def read_note(path: str) -> str:
    return (VAULT_PATH / path).read_text(encoding="utf-8")

def write_note(path: str, content: str) -> None:
    full = VAULT_PATH / path
    full.parent.mkdir(parents=True, exist_ok=True)
    full.write_text(content, encoding="utf-8")

def append_note(path: str, text: str) -> None:
    full = VAULT_PATH / path
    full.parent.mkdir(parents=True, exist_ok=True)
    if full.exists():
        existing = full.read_text(encoding="utf-8")
        full.write_text(existing + "\n" + text, encoding="utf-8")
    else:
        full.write_text(text, encoding="utf-8")

def list_notes() -> list[Path]:
    return list(VAULT_PATH.rglob("*.md"))

def extract_wikilinks(content: str) -> list[str]:
    return re.findall(r'\[\[([^\]|]+)\]\]', content)
```

- [ ] **Step 4 : Vérifier que les tests passent**

```bash
pytest tests/test_manager.py -v
```

Attendu : `8 passed`.

- [ ] **Step 5 : Commit**

```bash
git add vault/manager.py tests/test_manager.py
git commit -m "feat: vault manager (read/write/append/list/wikilinks)"
```

---

### Task 3 : Vault Indexer

**Files:**
- Create: `vault/indexer.py`
- Create: `tests/test_indexer.py`

- [ ] **Step 1 : Écrire les tests (chunk_text uniquement, sans I/O)**

```python
# tests/test_indexer.py
from vault.indexer import chunk_text

def test_chunk_text_short():
    words = ["word"] * 10
    text = " ".join(words)
    chunks = chunk_text(text, size=5, overlap=1)
    assert len(chunks) == 3  # [0:5], [4:9], [8:10]

def test_chunk_text_exact_fit():
    text = " ".join(["word"] * 5)
    chunks = chunk_text(text, size=5, overlap=0)
    assert len(chunks) == 1
    assert chunks[0] == text

def test_chunk_text_empty():
    assert chunk_text("") == []

def test_chunk_text_overlap_content():
    words = ["a", "b", "c", "d", "e"]
    text = " ".join(words)
    chunks = chunk_text(text, size=3, overlap=1)
    # [a b c], [c d e]
    assert "a b c" in chunks[0]
    assert "c d e" in chunks[1]
```

- [ ] **Step 2 : Vérifier que les tests échouent**

```bash
pytest tests/test_indexer.py -v
```

Attendu : `ImportError`.

- [ ] **Step 3 : Implémenter vault/indexer.py**

```python
# vault/indexer.py
import ollama
import chromadb
from pathlib import Path
from config import VAULT_PATH, CHROMA_PATH, EMBED_MODEL, OLLAMA_HOST, CHUNK_SIZE, CHUNK_OVERLAP, TOP_K

def chunk_text(text: str, size: int = CHUNK_SIZE, overlap: int = CHUNK_OVERLAP) -> list[str]:
    words = text.split()
    if not words:
        return []
    chunks = []
    step = size - overlap
    for i in range(0, len(words), step):
        chunks.append(" ".join(words[i:i + size]))
        if i + size >= len(words):
            break
    return chunks

def _get_collection():
    CHROMA_PATH.mkdir(parents=True, exist_ok=True)
    client = chromadb.PersistentClient(path=str(CHROMA_PATH))
    return client.get_or_create_collection("vault")

def _embed(texts: list[str]) -> list[list[float]]:
    client = ollama.Client(host=OLLAMA_HOST)
    return [client.embeddings(model=EMBED_MODEL, prompt=t)["embedding"] for t in texts]

def index_note(path: Path) -> None:
    content = path.read_text(encoding="utf-8")
    chunks = chunk_text(content)
    if not chunks:
        return
    col = _get_collection()
    rel = str(path.relative_to(VAULT_PATH))
    embeddings = _embed(chunks)
    ids = [f"{rel}::{i}" for i in range(len(chunks))]
    col.upsert(
        ids=ids,
        embeddings=embeddings,
        documents=chunks,
        metadatas=[{"path": rel} for _ in chunks],
    )

def index_all() -> None:
    for note in VAULT_PATH.rglob("*.md"):
        index_note(note)

def search(query: str, top_k: int = TOP_K) -> list[dict]:
    col = _get_collection()
    emb = _embed([query])[0]
    results = col.query(query_embeddings=[emb], n_results=top_k)
    if not results["documents"][0]:
        return []
    return [
        {"path": m["path"], "content": d}
        for m, d in zip(results["metadatas"][0], results["documents"][0])
    ]
```

- [ ] **Step 4 : Vérifier que les tests passent**

```bash
pytest tests/test_indexer.py -v
```

Attendu : `4 passed`.

- [ ] **Step 5 : Commit**

```bash
git add vault/indexer.py tests/test_indexer.py
git commit -m "feat: vault indexer (chunk, embed, ChromaDB search)"
```

---

### Task 4 : Memory RAG

**Files:**
- Create: `agent/memory.py`
- Create: `tests/test_memory.py`

- [ ] **Step 1 : Écrire les tests**

```python
# tests/test_memory.py
from unittest.mock import patch

SEARCH_RESULTS = [
    {"path": "memory/code.md", "content": "Python style simple [[user]]"},
    {"path": "memory/user.md",  "content": "Joris, dev Go/Python"},
]

def test_get_context_returns_content(tmp_vault):
    (tmp_vault / "memory" / "user.md").write_text("# User\nJoris", encoding="utf-8")
    with patch("agent.memory.search", return_value=SEARCH_RESULTS), \
         patch("agent.memory.VAULT_PATH", tmp_vault):
        from agent.memory import get_context
        ctx = get_context("code python")
    assert "memory/code.md" in ctx
    assert "Python style simple" in ctx

def test_get_context_follows_wikilinks(tmp_vault):
    (tmp_vault / "memory").mkdir(exist_ok=True)
    (tmp_vault / "memory" / "user.md").write_text("# User\nJoris", encoding="utf-8")
    with patch("agent.memory.search", return_value=SEARCH_RESULTS), \
         patch("agent.memory.VAULT_PATH", tmp_vault):
        from agent.memory import get_context
        ctx = get_context("code python")
    assert "Joris" in ctx  # contenu de user.md suivi via [[user]]

def test_get_context_ignores_missing_wikilinks(tmp_vault):
    results = [{"path": "memory/code.md", "content": "voir [[inexistant]]"}]
    with patch("agent.memory.search", return_value=results), \
         patch("agent.memory.VAULT_PATH", tmp_vault):
        from agent.memory import get_context
        ctx = get_context("code")  # ne doit pas lever d'exception
    assert "memory/code.md" in ctx

def test_get_context_no_duplicate_paths(tmp_vault):
    results = [
        {"path": "memory/user.md", "content": "Joris [[user]]"},  # [[user]] → user.md déjà vu
    ]
    (tmp_vault / "memory" / "user.md").write_text("# User", encoding="utf-8")
    with patch("agent.memory.search", return_value=results), \
         patch("agent.memory.VAULT_PATH", tmp_vault):
        from agent.memory import get_context
        ctx = get_context("user")
    assert ctx.count("memory/user.md") == 1  # pas de doublon
```

- [ ] **Step 2 : Vérifier que les tests échouent**

```bash
pytest tests/test_memory.py -v
```

Attendu : `ImportError`.

- [ ] **Step 3 : Implémenter agent/memory.py**

```python
# agent/memory.py
from pathlib import Path
from config import VAULT_PATH, TOP_K
from vault.indexer import search
from vault.manager import extract_wikilinks

def get_context(query: str) -> str:
    results = search(query, top_k=TOP_K)
    seen: set[str] = set()
    chunks: list[str] = []

    for r in results:
        path = r["path"]
        if path in seen:
            continue
        seen.add(path)
        chunks.append(f"### {path}\n{r['content']}")

        for link in extract_wikilinks(r["content"]):
            link_path = f"memory/{link}.md"
            if link_path in seen:
                continue
            seen.add(link_path)
            full = VAULT_PATH / link_path
            if full.exists():
                content = full.read_text(encoding="utf-8")[:1000]
                chunks.append(f"### {link_path}\n{content}")

    return "\n\n---\n\n".join(chunks)
```

- [ ] **Step 4 : Vérifier que les tests passent**

```bash
pytest tests/test_memory.py -v
```

Attendu : `4 passed`.

- [ ] **Step 5 : Commit**

```bash
git add agent/memory.py tests/test_memory.py
git commit -m "feat: memory RAG (search + wikilink graph traversal)"
```

---

### Task 5 : Outils Internet

**Files:**
- Create: `agent/tools.py`
- Create: `tests/test_tools.py`

- [ ] **Step 1 : Écrire les tests**

```python
# tests/test_tools.py
from unittest.mock import patch, MagicMock

def test_web_search_returns_formatted_results():
    mock_results = [
        {"title": "CVE-2024-1234", "href": "https://nvd.nist.gov/1234", "body": "Buffer overflow"},
        {"title": "Exploit DB", "href": "https://exploit-db.com/5678", "body": "PoC disponible"},
    ]
    with patch("agent.tools.DDGS") as mock_ddgs:
        mock_ddgs.return_value.__enter__.return_value.text.return_value = mock_results
        from agent.tools import web_search
        result = web_search("CVE-2024-1234")
    assert "CVE-2024-1234" in result
    assert "https://nvd.nist.gov/1234" in result
    assert "Buffer overflow" in result

def test_web_search_empty_results():
    with patch("agent.tools.DDGS") as mock_ddgs:
        mock_ddgs.return_value.__enter__.return_value.text.return_value = []
        from agent.tools import web_search
        result = web_search("requête vide")
    assert result == ""

def test_fetch_url_extracts_text():
    mock_response = MagicMock()
    mock_response.text = "<html><body><h1>Titre</h1><p>Contenu important</p></body></html>"
    with patch("agent.tools.httpx.get", return_value=mock_response):
        from agent.tools import fetch_url
        result = fetch_url("https://example.com")
    assert "Titre" in result
    assert "Contenu important" in result

def test_fetch_url_truncates_at_max_chars():
    mock_response = MagicMock()
    mock_response.text = "<html><body>" + "x" * 20000 + "</body></html>"
    with patch("agent.tools.httpx.get", return_value=mock_response):
        from agent.tools import fetch_url
        result = fetch_url("https://example.com", max_chars=8000)
    assert len(result) <= 8000
```

- [ ] **Step 2 : Vérifier que les tests échouent**

```bash
pytest tests/test_tools.py -v
```

Attendu : `ImportError`.

- [ ] **Step 3 : Implémenter agent/tools.py**

```python
# agent/tools.py
import httpx
import html2text
from duckduckgo_search import DDGS

def web_search(query: str, n: int = 5) -> str:
    with DDGS() as ddgs:
        results = list(ddgs.text(query, max_results=n))
    lines = [f"- [{r['title']}]({r['href']})\n  {r['body']}" for r in results]
    return "\n".join(lines)

def fetch_url(url: str, max_chars: int = 8000) -> str:
    resp = httpx.get(url, follow_redirects=True, timeout=10)
    resp.raise_for_status()
    h = html2text.HTML2Text()
    h.ignore_links = False
    h.body_width = 0
    return h.handle(resp.text)[:max_chars]
```

- [ ] **Step 4 : Vérifier que les tests passent**

```bash
pytest tests/test_tools.py -v
```

Attendu : `4 passed`.

- [ ] **Step 5 : Commit**

```bash
git add agent/tools.py tests/test_tools.py
git commit -m "feat: internet tools (web_search + fetch_url)"
```

---

### Task 6 : Core Agent (ReAct Loop)

**Files:**
- Create: `agent/core.py`
- Create: `tests/test_core.py`

- [ ] **Step 1 : Écrire les tests**

```python
# tests/test_core.py
from unittest.mock import patch, MagicMock, call

def _make_ollama_response(content: str):
    """Mock d'une réponse Ollama non-streamée."""
    return {"message": {"content": content}}

def _make_stream_chunks(content: str):
    """Mock d'un stream Ollama token par token."""
    return [{"message": {"content": c}} for c in content]

def test_run_agent_simple_response(mocker):
    mocker.patch("agent.core.get_context", return_value="contexte mémoire")
    mock_client = MagicMock()
    mock_client.chat.return_value = _make_stream_chunks("Bonjour !")
    mocker.patch("agent.core.ollama.Client", return_value=mock_client)
    mocker.patch("agent.core._save_memory")

    from agent.core import run_agent
    result = run_agent([{"role": "user", "content": "Salut"}])
    assert result == "Bonjour !"

def test_run_agent_calls_web_search(mocker):
    mocker.patch("agent.core.get_context", return_value="")
    mock_client = MagicMock()
    # Premier appel : l'IA demande un web_search
    # Deuxième appel : réponse finale
    mock_client.chat.side_effect = [
        _make_stream_chunks('ACTION: web_search("CVE-2024-1234")'),
        _make_stream_chunks("Voici les résultats du CVE."),
    ]
    mocker.patch("agent.core.ollama.Client", return_value=mock_client)
    mocker.patch("agent.core.web_search", return_value="CVE résultats")
    mocker.patch("agent.core._save_memory")

    from agent.core import run_agent
    result = run_agent([{"role": "user", "content": "Infos sur CVE-2024-1234"}])
    assert result == "Voici les résultats du CVE."

def test_run_agent_calls_fetch_url(mocker):
    mocker.patch("agent.core.get_context", return_value="")
    mock_client = MagicMock()
    mock_client.chat.side_effect = [
        _make_stream_chunks('ACTION: fetch_url("https://example.com")'),
        _make_stream_chunks("Contenu de la page."),
    ]
    mocker.patch("agent.core.ollama.Client", return_value=mock_client)
    mocker.patch("agent.core.fetch_url", return_value="contenu html parsé")
    mocker.patch("agent.core._save_memory")

    from agent.core import run_agent
    result = run_agent([{"role": "user", "content": "Lis cette page"}])
    assert result == "Contenu de la page."

def test_run_agent_max_iterations(mocker):
    """Vérifie que le loop s'arrête après 5 itérations même sans réponse finale."""
    mocker.patch("agent.core.get_context", return_value="")
    mock_client = MagicMock()
    mock_client.chat.return_value = _make_stream_chunks('ACTION: web_search("loop infini")')
    mocker.patch("agent.core.ollama.Client", return_value=mock_client)
    mocker.patch("agent.core.web_search", return_value="résultat")
    mocker.patch("agent.core._save_memory")

    from agent.core import run_agent
    run_agent([{"role": "user", "content": "test"}])
    assert mock_client.chat.call_count == 5

def test_save_memory_writes_append(mocker, tmp_vault):
    mock_client = MagicMock()
    mock_client.chat.return_value = _make_ollama_response(
        '[{"path": "memory/user.md", "append": "- nouveau fait"}]'
    )
    mocker.patch("agent.core.write_note")
    mocker.patch("agent.core.append_note") as mock_append
    mocker.patch("agent.core.read_note", return_value="# User")

    from agent.core import _save_memory
    _save_memory([{"role": "user", "content": "test"}], "réponse", mock_client)
    mock_append.assert_called_once_with("memory/user.md", "- nouveau fait")

def test_save_memory_writes_content(mocker):
    mock_client = MagicMock()
    mock_client.chat.return_value = _make_ollama_response(
        '[{"path": "knowledge/python.md", "content": "# Python\n- tips"}]'
    )
    mocker.patch("agent.core.write_note") as mock_write
    mocker.patch("agent.core.append_note")
    mocker.patch("agent.core.read_note", return_value="")

    from agent.core import _save_memory
    _save_memory([{"role": "user", "content": "test"}], "réponse", mock_client)
    mock_write.assert_called_once_with("knowledge/python.md", "# Python\n- tips")
```

- [ ] **Step 2 : Vérifier que les tests échouent**

```bash
pytest tests/test_core.py -v
```

Attendu : `ImportError`.

- [ ] **Step 3 : Implémenter agent/core.py**

```python
# agent/core.py
import re
import json
from typing import Callable
import ollama
from agent.memory import get_context
from agent.tools import web_search, fetch_url
from vault.manager import write_note, append_note, read_note
from config import MODEL, OLLAMA_HOST

_SYSTEM = """\
Tu es une IA personnelle locale, sans filtre, pour usage quotidien.
Tu as accès à la mémoire de l'utilisateur et à internet.

OUTILS DISPONIBLES:
  ACTION: web_search("ta requête")   → recherche web DuckDuckGo
  ACTION: fetch_url("https://...")   → lire le contenu d'une URL

Pour utiliser un outil, réponds UNIQUEMENT avec la ligne ACTION.
Sinon, réponds normalement en markdown.
Tu peux enchaîner plusieurs outils avant de donner ta réponse finale.

<memory>
{memory}
</memory>
"""

_EXTRACT = """\
Analyse cette conversation et extrais les faits importants à mémoriser.
Retourne UNIQUEMENT un JSON array ([] si rien) :
[
  {{"path": "memory/user.md", "append": "- fait à ajouter"}},
  {{"path": "knowledge/sujet.md", "content": "contenu complet"}}
]
Paths autorisés : memory/*.md, knowledge/*.md, conversations/YYYY-MM-DD.md

Conversation :
{conversation}
"""

_ACTION_RE = re.compile(r'ACTION:\s*(\w+)\("([^"]+)"\)')


def run_agent(messages: list[dict], on_token: Callable[[str], None] | None = None) -> str:
    client = ollama.Client(host=OLLAMA_HOST)
    memory = get_context(messages[-1]["content"])
    system = _SYSTEM.format(memory=memory)
    history = [{"role": "system", "content": system}] + messages

    response = ""
    for _ in range(5):
        response = ""
        for chunk in client.chat(model=MODEL, messages=history, stream=True):
            token = chunk["message"]["content"]
            response += token
            if on_token:
                on_token(token)

        match = _ACTION_RE.search(response)
        if not match:
            break

        tool, arg = match.group(1), match.group(2)
        result = web_search(arg) if tool == "web_search" else fetch_url(arg) if tool == "fetch_url" else ""
        history.append({"role": "assistant", "content": response})
        history.append({"role": "user", "content": f"RESULT:\n{result}"})

    _save_memory(messages, response, client)
    return response


def _save_memory(messages: list[dict], response: str, client) -> None:
    conv = "\n".join(f"{m['role']}: {m['content']}" for m in messages)
    conv += f"\nassistant: {response}"
    result = client.chat(
        model=MODEL,
        messages=[{"role": "user", "content": _EXTRACT.format(conversation=conv[-3000:])}],
    )
    text = result["message"]["content"]
    json_match = re.search(r'\[.*?\]', text, re.DOTALL)
    if not json_match:
        return
    try:
        facts = json.loads(json_match.group())
    except json.JSONDecodeError:
        return
    for fact in facts:
        path = fact.get("path", "")
        if not path:
            continue
        if "append" in fact:
            append_note(path, fact["append"])
        elif "content" in fact:
            write_note(path, fact["content"])
```

- [ ] **Step 4 : Vérifier que les tests passent**

```bash
pytest tests/test_core.py -v
```

Attendu : `6 passed`.

- [ ] **Step 5 : Commit**

```bash
git add agent/core.py tests/test_core.py
git commit -m "feat: core agent ReAct loop + memory extraction"
```

---

### Task 7 : TUI Textual

**Files:**
- Create: `tui/app.py`

> Pas de tests unitaires pour le TUI (widgets Textual difficiles à mocker sans app headless). On teste manuellement à l'étape 4.

- [ ] **Step 1 : Implémenter tui/app.py**

```python
# tui/app.py
from textual.app import App, ComposeResult
from textual.widgets import Header, Footer, Input, RichLog, Static
from textual.containers import Vertical
from textual.binding import Binding
from textual import work
from agent.core import run_agent

class PersonalAI(App):
    TITLE = "Personal AI"
    CSS = """
    #history {
        height: 1fr;
        border: solid #00ff76;
        padding: 1 2;
    }
    #streaming {
        height: auto;
        min-height: 1;
        padding: 0 2;
        color: #aaffaa;
    }
    #input {
        margin: 1 0 0 0;
    }
    """
    BINDINGS = [
        Binding("escape", "quit", "Quitter"),
        Binding("ctrl+c", "quit", "Quitter"),
    ]

    def __init__(self):
        super().__init__()
        self.messages: list[dict] = []

    def compose(self) -> ComposeResult:
        yield Header(show_clock=True)
        with Vertical():
            yield RichLog(id="history", markup=True, wrap=True, highlight=True)
            yield Static("", id="streaming", markup=True)
        yield Input(placeholder="Tape un message... (/clear /memory /search <q> /note <texte>)", id="input")
        yield Footer()

    def on_mount(self) -> None:
        self.query_one("#input", Input).focus()
        self.query_one("#history", RichLog).write(
            "[bold green]IA personnelle prête. Dolphin-Mixtral + mémoire Obsidian.[/bold green]"
        )

    def on_input_submitted(self, event: Input.Submitted) -> None:
        text = event.value.strip()
        if not text:
            return
        event.input.value = ""

        if text.startswith("/"):
            self._handle_command(text)
            return

        self.query_one("#history", RichLog).write(f"\n[bold cyan]Toi:[/bold cyan] {text}")
        self.messages.append({"role": "user", "content": text})
        self._respond()

    def _handle_command(self, cmd: str) -> None:
        log = self.query_one("#history", RichLog)
        if cmd == "/clear":
            self.messages = []
            log.clear()
            log.write("[bold green]Conversation réinitialisée.[/bold green]")
        elif cmd.startswith("/search "):
            query = cmd[8:].strip()
            from agent.memory import get_context
            ctx = get_context(query) or "Aucun résultat dans la mémoire."
            log.write(f"\n[bold yellow]Mémoire — '{query}':[/bold yellow]\n{ctx[:600]}")
        elif cmd == "/memory" and self.messages:
            from agent.memory import get_context
            ctx = get_context(self.messages[-1]["content"]) or "Aucun contexte."
            log.write(f"\n[bold yellow]Contexte mémoire :[/bold yellow]\n{ctx[:600]}")
        elif cmd.startswith("/note "):
            from vault.manager import append_note
            from datetime import datetime
            note_text = cmd[6:].strip()
            today = datetime.now().strftime("%Y-%m-%d")
            append_note(f"conversations/{today}.md", f"- {note_text}")
            log.write(f"[bold green]Note enregistrée.[/bold green]")

    @work(thread=True)
    def _respond(self) -> None:
        log = self.query_one("#history", RichLog)
        streaming = self.query_one("#streaming", Static)
        buffer: list[str] = []

        def on_token(token: str) -> None:
            buffer.append(token)
            self.call_from_thread(
                streaming.update,
                f"[bold green]IA:[/bold green] {''.join(buffer)}"
            )

        try:
            response = run_agent(self.messages, on_token=on_token)
            self.messages.append({"role": "assistant", "content": response})
            self.call_from_thread(log.write, f"\n[bold green]IA:[/bold green] {response}")
            self.call_from_thread(streaming.update, "")
        except Exception as e:
            self.call_from_thread(log.write, f"\n[bold red]Erreur : {e}[/bold red]")
            self.call_from_thread(streaming.update, "")
```

- [ ] **Step 2 : Test manuel rapide (sans vault indexée)**

```bash
source ~/.personal-ai/venv/bin/activate
cd ~/Projects/personal-ai
python -c "from tui.app import PersonalAI; print('OK import TUI')"
```

Attendu : `OK import TUI` sans erreur.

- [ ] **Step 3 : Commit**

```bash
git add tui/app.py
git commit -m "feat: Textual TUI with streaming, /commands"
```

---

### Task 8 : Entry Point + Install

**Files:**
- Create: `main.py`
- Create: `install.sh`

- [ ] **Step 1 : Créer main.py**

```python
# main.py
from vault.indexer import index_all
from tui.app import PersonalAI

def main():
    print("Indexation de la vault...", flush=True)
    index_all()
    PersonalAI().run()

if __name__ == "__main__":
    main()
```

- [ ] **Step 2 : Créer install.sh**

```bash
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
KEYBINDS="$HOME/.config/hypr/Keybinds.conf"
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
windowrulev2 = float, class:personal-ai
windowrulev2 = size 900 600, class:personal-ai
windowrulev2 = center, class:personal-ai
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
```

- [ ] **Step 3 : Rendre install.sh exécutable**

```bash
chmod +x install.sh
```

- [ ] **Step 4 : Lancer l'installation**

```bash
cd ~/Projects/personal-ai
./install.sh
```

Attendu : chaque étape s'affiche sans erreur. `nomic-embed-text` téléchargé, venv prêt, vault initialisée.

- [ ] **Step 5 : Lancer la suite de tests complète**

```bash
source ~/.personal-ai/venv/bin/activate
pytest tests/ -v
```

Attendu : **22 passed** (tous les tests).

- [ ] **Step 6 : Test de fumée — lancer l'IA**

```bash
source ~/.personal-ai/venv/bin/activate
cd ~/Projects/personal-ai
python main.py
```

Attendu : `Indexation de la vault...` puis le TUI s'ouvre. Taper "Bonjour, qui suis-je ?" et vérifier que l'IA répond en utilisant `memory/user.md`.

- [ ] **Step 7 : Commit final**

```bash
git add main.py install.sh
git commit -m "feat: entry point + one-shot installer + Hyprland keybind"
```

---

## Récapitulatif des commits

| Commit | Contenu |
|--------|---------|
| `feat: project setup` | requirements, config, venv, conftest |
| `feat: vault manager` | read/write/append/list/wikilinks |
| `feat: vault indexer` | chunk, embed, ChromaDB |
| `feat: memory RAG` | search + graph traversal |
| `feat: internet tools` | web_search + fetch_url |
| `feat: core agent` | ReAct loop + mémoire post-réponse |
| `feat: TUI` | Textual chat streamé + commandes |
| `feat: entry point + install` | main.py + install.sh + Hyprland |

## Commandes utiles post-installation

```bash
ai                          # lancer l'IA
/clear                      # réinitialiser la conversation
/memory                     # voir le contexte mémoire du dernier message
/search ethical hacking     # chercher dans la vault
/note J'ai appris X         # forcer l'écriture d'une note
```
