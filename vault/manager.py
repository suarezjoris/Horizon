# vault/manager.py
import re
from pathlib import Path
from config import VAULT_PATH

def read_note(path: str) -> str:
    return (VAULT_PATH / path).read_text(encoding="utf-8")

def _reindex(path: Path) -> None:
    try:
        from vault.indexer import index_note
        index_note(path)
    except Exception:
        pass

def write_note(path: str, content: str) -> None:
    full = VAULT_PATH / path
    full.parent.mkdir(parents=True, exist_ok=True)
    full.write_text(content, encoding="utf-8")
    _reindex(full)

def append_note(path: str, text: str) -> None:
    full = VAULT_PATH / path
    full.parent.mkdir(parents=True, exist_ok=True)
    if full.exists():
        existing = full.read_text(encoding="utf-8")
        full.write_text(existing + "\n" + text, encoding="utf-8")
    else:
        full.write_text(text, encoding="utf-8")
    _reindex(full)

def list_notes() -> list[Path]:
    return list(VAULT_PATH.rglob("*.md"))

def extract_wikilinks(content: str) -> list[str]:
    return re.findall(r'\[\[([^\]|]+)\]\]', content)
